use axum::http::{HeaderMap, header};
use base64::{Engine, engine::general_purpose::STANDARD};
use depo_core::git::RepoId;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};
use serde::Deserialize;

use crate::AuthMode;

pub const GIT_HTTP_USERNAME: &str = "git";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitAccess {
    Read,
    Write,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitActor {
    pub subject: String,
}

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("authentication is required")]
    Missing,
    #[error("authorization header is malformed")]
    Malformed,
    #[error("authorization scheme is not supported")]
    UnsupportedScheme,
    #[error("credentials are not valid")]
    InvalidCredentials,
    #[error("token does not grant access to this repository")]
    RepoForbidden,
    #[error("token does not grant the required scope")]
    ScopeForbidden,
    #[error("auth verifier is not configured correctly")]
    VerifierMisconfigured,
}

#[derive(Debug, Deserialize)]
struct GitClaims {
    sub: Option<String>,
    repo: String,
    scopes: Vec<String>,
}

pub fn authenticate_git(
    mode: &AuthMode,
    headers: &HeaderMap,
    repo: &RepoId,
    access: GitAccess,
) -> Result<GitActor, AuthError> {
    let credential = parse_authorization(headers)?;

    match mode {
        AuthMode::Local => {
            let token = credential.basic_token()?;
            if token.is_empty() {
                return Err(AuthError::InvalidCredentials);
            }
            Ok(GitActor {
                subject: "local".to_owned(),
            })
        }
        AuthMode::Jwt { public_key_pem } => {
            let token = credential.token()?;
            let key = DecodingKey::from_ec_pem(public_key_pem.as_bytes())
                .map_err(|_| AuthError::VerifierMisconfigured)?;
            let mut validation = Validation::new(Algorithm::ES256);
            validation.set_required_spec_claims(&["exp", "repo", "scopes"]);

            let decoded = decode::<GitClaims>(&token, &key, &validation)
                .map_err(|_| AuthError::InvalidCredentials)?;
            if decoded.claims.repo != repo.as_full_name() {
                return Err(AuthError::RepoForbidden);
            }
            if !has_scope(&decoded.claims.scopes, access) {
                return Err(AuthError::ScopeForbidden);
            }

            Ok(GitActor {
                subject: decoded.claims.sub.unwrap_or_else(|| "token".to_owned()),
            })
        }
    }
}

fn has_scope(scopes: &[String], access: GitAccess) -> bool {
    match access {
        GitAccess::Read => scopes
            .iter()
            .any(|scope| scope == "git:read" || scope == "git:write"),
        GitAccess::Write => scopes.iter().any(|scope| scope == "git:write"),
    }
}

enum Credential {
    Basic { username: String, token: String },
    Bearer(String),
}

impl Credential {
    fn basic_token(self) -> Result<String, AuthError> {
        match self {
            Self::Basic { username, token } if username == GIT_HTTP_USERNAME => Ok(token),
            Self::Basic { .. } => Err(AuthError::InvalidCredentials),
            Self::Bearer(_) => Err(AuthError::UnsupportedScheme),
        }
    }

    fn token(self) -> Result<String, AuthError> {
        match self {
            Self::Basic { username, token } if username == GIT_HTTP_USERNAME => Ok(token),
            Self::Basic { .. } => Err(AuthError::InvalidCredentials),
            Self::Bearer(token) => Ok(token),
        }
    }
}

fn parse_authorization(headers: &HeaderMap) -> Result<Credential, AuthError> {
    let value = headers
        .get(header::AUTHORIZATION)
        .ok_or(AuthError::Missing)?
        .to_str()
        .map_err(|_| AuthError::Malformed)?;

    if let Some(encoded) = value.strip_prefix("Basic ") {
        let decoded = STANDARD.decode(encoded).map_err(|_| AuthError::Malformed)?;
        let decoded = String::from_utf8(decoded).map_err(|_| AuthError::Malformed)?;
        let (username, token) = decoded.split_once(':').ok_or(AuthError::Malformed)?;
        return Ok(Credential::Basic {
            username: username.to_owned(),
            token: token.to_owned(),
        });
    }

    if let Some(token) = value.strip_prefix("Bearer ") {
        if token.trim().is_empty() {
            return Err(AuthError::InvalidCredentials);
        }
        return Ok(Credential::Bearer(token.to_owned()));
    }

    Err(AuthError::UnsupportedScheme)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderValue, header};
    use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
    use serde::Serialize;

    const PRIVATE_KEY_PEM: &str = r#"-----BEGIN PRIVATE KEY-----
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgjKVT/iQOuYiaop5y
bHMtVpU+xRI/cvhVCpLV12GTaPChRANCAASHG4E3FBb4s2MbPSNvHuxmAE8UMnAn
CIjAx97UU/A4B5d5bW/D0cI+SjnqL2Bb6sNygdNvz6Q9/Xfs5pCyVosk
-----END PRIVATE KEY-----"#;

    const PUBLIC_KEY_PEM: &str = r#"-----BEGIN PUBLIC KEY-----
MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEhxuBNxQW+LNjGz0jbx7sZgBPFDJw
JwiIwMfe1FPwOAeXeW1vw9HCPko56i9gW+rDcoHTb8+kPf137OaQslaLJA==
-----END PUBLIC KEY-----"#;

    #[derive(Serialize)]
    struct TestClaims {
        sub: String,
        repo: String,
        scopes: Vec<String>,
        exp: usize,
    }

    #[test]
    fn local_git_auth_requires_basic_token_username() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Basic Z2l0OmxvY2Fs"),
        );
        let repo = RepoId::parse("kian", "depo").unwrap();

        let actor = authenticate_git(&AuthMode::Local, &headers, &repo, GitAccess::Write).unwrap();

        assert_eq!(actor.subject, "local");
    }

    #[test]
    fn local_git_auth_rejects_missing_credentials() {
        let headers = HeaderMap::new();
        let repo = RepoId::parse("kian", "depo").unwrap();

        let error = authenticate_git(&AuthMode::Local, &headers, &repo, GitAccess::Read)
            .expect_err("missing auth should be rejected");

        assert!(matches!(error, AuthError::Missing));
    }

    #[test]
    fn jwt_git_auth_enforces_repo_and_scope() {
        let repo = RepoId::parse("kian", "depo").unwrap();
        let token = encode(
            &Header::new(Algorithm::ES256),
            &TestClaims {
                sub: "kian".to_owned(),
                repo: repo.as_full_name(),
                scopes: vec!["git:write".to_owned()],
                exp: 4_102_444_800,
            },
            &EncodingKey::from_ec_pem(PRIVATE_KEY_PEM.as_bytes()).unwrap(),
        )
        .unwrap();
        let mut headers = HeaderMap::new();
        let encoded = STANDARD.encode(format!("{GIT_HTTP_USERNAME}:{token}"));
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Basic {encoded}")).unwrap(),
        );

        let actor = authenticate_git(
            &AuthMode::Jwt {
                public_key_pem: PUBLIC_KEY_PEM.to_owned(),
            },
            &headers,
            &repo,
            GitAccess::Read,
        )
        .unwrap();

        assert_eq!(actor.subject, "kian");

        let read_only = encode(
            &Header::new(Algorithm::ES256),
            &TestClaims {
                sub: "kian".to_owned(),
                repo: repo.as_full_name(),
                scopes: vec!["git:read".to_owned()],
                exp: 4_102_444_800,
            },
            &EncodingKey::from_ec_pem(PRIVATE_KEY_PEM.as_bytes()).unwrap(),
        )
        .unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {read_only}")).unwrap(),
        );

        let error = authenticate_git(
            &AuthMode::Jwt {
                public_key_pem: PUBLIC_KEY_PEM.to_owned(),
            },
            &headers,
            &repo,
            GitAccess::Write,
        )
        .expect_err("read-only token must not authorize push");

        assert!(matches!(error, AuthError::ScopeForbidden));
    }
}
