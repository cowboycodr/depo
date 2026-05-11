use std::{path::PathBuf, time::Duration};

use axum::{
    body::{Body, Bytes, to_bytes},
    extract::State,
    http::{HeaderMap, HeaderName, HeaderValue, Method, Request, Response, StatusCode, header},
    response::IntoResponse,
};
use depo_core::git::{BareRepository, GitCommandRequest, RepoId, RepoName, RepositoryError};

use crate::{
    AppState,
    auth::{AuthError, GitAccess, authenticate_git},
    db,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GitService {
    UploadPack,
    ReceivePack,
}

impl GitService {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "git-upload-pack" => Some(Self::UploadPack),
            "git-receive-pack" => Some(Self::ReceivePack),
            _ => None,
        }
    }

    fn rpc_path(self) -> &'static str {
        match self {
            Self::UploadPack => "git-upload-pack",
            Self::ReceivePack => "git-receive-pack",
        }
    }

    fn access(self) -> GitAccess {
        match self {
            Self::UploadPack => GitAccess::Read,
            Self::ReceivePack => GitAccess::Write,
        }
    }
}

struct GitHttpTarget {
    repo_id: RepoId,
    operation_path: String,
    service: GitService,
}

pub async fn handle(
    State(state): State<AppState>,
    request: Request<Body>,
) -> Result<Response<Body>, GitHttpError> {
    let (parts, body) = request.into_parts();
    let target = parse_target(
        parts.uri.path(),
        &parts.method,
        parts.uri.query().unwrap_or(""),
    )?;

    let actor = authenticate_git(
        &state.auth_mode,
        &parts.headers,
        &target.repo_id,
        target.service.access(),
    )?;
    let record = db::get_repository(
        &state.db,
        target.repo_id.owner().as_str(),
        target.repo_id.name().as_str(),
    )
    .await?
    .ok_or_else(|| {
        GitHttpError::not_found(format!("Repository {} does not exist.", target.repo_id))
    })?;

    let repo_path = PathBuf::from(&record.storage_path);
    let expected_path = state.storage.bare_repo_path(&target.repo_id);
    if repo_path != expected_path {
        return Err(GitHttpError::internal(
            "Repository storage path does not match the configured storage root.",
        ));
    }
    BareRepository::open(target.repo_id.clone(), repo_path, state.git.clone())?;

    let body_bytes = to_bytes(body, state.git_http_body_limit)
        .await
        .map_err(|_| GitHttpError::payload_too_large(state.git_http_body_limit))?;
    let output = run_http_backend(
        &state,
        &target,
        &parts.method,
        parts.uri.query().unwrap_or(""),
        &parts.headers,
        body_bytes,
        actor.subject,
    )
    .await?;

    parse_cgi_response(output.stdout)
}

fn parse_target(
    uri_path: &str,
    method: &Method,
    query: &str,
) -> Result<GitHttpTarget, GitHttpError> {
    let path = uri_path.trim_start_matches('/');
    let Some((owner, path)) = path.split_once('/') else {
        return Err(GitHttpError::not_found("Unsupported Depo route."));
    };
    let Some((repo_name, operation_path)) = path.split_once(".git/") else {
        return Err(GitHttpError::not_found(
            "Git repository URL must end in .git.",
        ));
    };
    let repo_name = RepoName::parse(repo_name)?;
    let repo_id = RepoId::new(depo_core::git::RepoOwner::parse(owner)?, repo_name);

    let service = match (method, operation_path) {
        (&Method::GET, "info/refs") => service_from_query(query).ok_or_else(|| {
            GitHttpError::bad_request("Git smart-HTTP discovery requires a service query.")
        })?,
        (&Method::POST, "git-upload-pack") => GitService::UploadPack,
        (&Method::POST, "git-receive-pack") => GitService::ReceivePack,
        _ => return Err(GitHttpError::not_found("Unsupported Git smart-HTTP path.")),
    };

    if operation_path != "info/refs" && operation_path != service.rpc_path() {
        return Err(GitHttpError::bad_request(
            "Git smart-HTTP service path does not match the requested service.",
        ));
    }

    Ok(GitHttpTarget {
        repo_id,
        operation_path: operation_path.to_owned(),
        service,
    })
}

fn service_from_query(query: &str) -> Option<GitService> {
    query.split('&').find_map(|pair| {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        if key == "service" {
            GitService::parse(value)
        } else {
            None
        }
    })
}

async fn run_http_backend(
    state: &AppState,
    target: &GitHttpTarget,
    method: &Method,
    query: &str,
    headers: &HeaderMap,
    body: Bytes,
    remote_user: String,
) -> Result<depo_core::git::GitCommandOutput, GitHttpError> {
    let path_info = format!(
        "/{}/{}.git/{}",
        target.repo_id.owner().as_str(),
        target.repo_id.name().as_str(),
        target.operation_path
    );
    let storage_root = path_to_env(state.storage.path())?;
    let content_length = body.len().to_string();

    let mut request = GitCommandRequest::new(["http-backend"])
        .env("GIT_PROJECT_ROOT", storage_root)
        .env("GIT_HTTP_EXPORT_ALL", "1")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("REQUEST_METHOD", method.as_str())
        .env("PATH_INFO", path_info)
        .env("QUERY_STRING", query)
        .env("REMOTE_USER", remote_user)
        .env("CONTENT_LENGTH", content_length)
        .timeout(Duration::from_secs(120));

    if let Some(content_type) = headers.get(header::CONTENT_TYPE) {
        request = request.env("CONTENT_TYPE", header_value_to_env(content_type)?);
    }
    if let Some(protocol) = headers.get("git-protocol") {
        let value = header_value_to_env(protocol)?;
        request = request
            .env("GIT_PROTOCOL", value.clone())
            .env("HTTP_GIT_PROTOCOL", value);
    }
    if !body.is_empty() {
        request = request.stdin(body.to_vec());
    }

    let git = state.git.clone();
    tokio::task::spawn_blocking(move || git.run(request))
        .await
        .map_err(|_| GitHttpError::internal("Git smart-HTTP task failed to join."))?
        .map_err(|error| GitHttpError::internal(format!("Git smart-HTTP failed: {error}")))
}

fn parse_cgi_response(output: Vec<u8>) -> Result<Response<Body>, GitHttpError> {
    let (header_bytes, body_bytes) = split_cgi_output(&output)?;
    let header_text = std::str::from_utf8(header_bytes)
        .map_err(|_| GitHttpError::internal("Git smart-HTTP returned non-UTF-8 headers."))?;
    let mut status = StatusCode::OK;
    let mut headers = HeaderMap::new();

    for raw_line in header_text.lines() {
        let line = raw_line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        let (name, value) = line.split_once(':').ok_or_else(|| {
            GitHttpError::internal(format!("Git smart-HTTP returned invalid header {line:?}."))
        })?;
        if name.eq_ignore_ascii_case("Status") {
            let code = value
                .trim()
                .split_whitespace()
                .next()
                .and_then(|value| value.parse::<u16>().ok())
                .ok_or_else(|| {
                    GitHttpError::internal("Git smart-HTTP returned invalid status header.")
                })?;
            status = StatusCode::from_u16(code).map_err(|_| {
                GitHttpError::internal("Git smart-HTTP returned invalid status code.")
            })?;
            continue;
        }

        let header_name = HeaderName::from_bytes(name.as_bytes()).map_err(|_| {
            GitHttpError::internal(format!("Git smart-HTTP returned invalid header {name:?}."))
        })?;
        let header_value = HeaderValue::from_str(value.trim()).map_err(|_| {
            GitHttpError::internal(format!(
                "Git smart-HTTP returned invalid value for header {name:?}."
            ))
        })?;
        headers.append(header_name, header_value);
    }

    let mut response = Response::builder().status(status);
    for (name, value) in headers.iter() {
        response = response.header(name.clone(), value.clone());
    }
    response
        .body(Body::from(body_bytes.to_vec()))
        .map_err(|_| GitHttpError::internal("Failed to build Git smart-HTTP response."))
}

fn split_cgi_output(output: &[u8]) -> Result<(&[u8], &[u8]), GitHttpError> {
    if let Some(index) = output.windows(4).position(|window| window == b"\r\n\r\n") {
        return Ok((&output[..index], &output[index + 4..]));
    }
    if let Some(index) = output.windows(2).position(|window| window == b"\n\n") {
        return Ok((&output[..index], &output[index + 2..]));
    }
    Err(GitHttpError::internal(
        "Git smart-HTTP response did not include CGI headers.",
    ))
}

fn path_to_env(path: &std::path::Path) -> Result<String, GitHttpError> {
    path.to_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| GitHttpError::internal("Repository storage path is not valid UTF-8."))
}

fn header_value_to_env(value: &HeaderValue) -> Result<String, GitHttpError> {
    value
        .to_str()
        .map(ToOwned::to_owned)
        .map_err(|_| GitHttpError::bad_request("Git request contains a non-UTF-8 header value."))
}

#[derive(Debug)]
pub struct GitHttpError {
    status: StatusCode,
    message: String,
    authenticate: bool,
}

impl GitHttpError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
            authenticate: false,
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
            authenticate: false,
        }
    }

    fn payload_too_large(limit: usize) -> Self {
        Self {
            status: StatusCode::PAYLOAD_TOO_LARGE,
            message: format!("Git request body exceeds the configured {limit} byte limit."),
            authenticate: false,
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
            authenticate: false,
        }
    }
}

impl IntoResponse for GitHttpError {
    fn into_response(self) -> Response<Body> {
        let mut response = Response::builder()
            .status(self.status)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8");
        if self.authenticate {
            response = response.header(header::WWW_AUTHENTICATE, "Basic realm=\"Depo Git\"");
        }
        response
            .body(Body::from(format!("{}\n", self.message)))
            .expect("static Git HTTP error response should be valid")
    }
}

impl From<AuthError> for GitHttpError {
    fn from(error: AuthError) -> Self {
        match error {
            AuthError::Missing => Self {
                status: StatusCode::UNAUTHORIZED,
                message: error.to_string(),
                authenticate: true,
            },
            AuthError::Malformed | AuthError::UnsupportedScheme | AuthError::InvalidCredentials => {
                Self {
                    status: StatusCode::UNAUTHORIZED,
                    message: error.to_string(),
                    authenticate: true,
                }
            }
            AuthError::RepoForbidden | AuthError::ScopeForbidden => Self {
                status: StatusCode::FORBIDDEN,
                message: error.to_string(),
                authenticate: false,
            },
            AuthError::VerifierMisconfigured => {
                Self::internal("Git authentication verifier is not configured correctly.")
            }
        }
    }
}

impl From<depo_core::git::IdError> for GitHttpError {
    fn from(error: depo_core::git::IdError) -> Self {
        Self::bad_request(error.to_string())
    }
}

impl From<RepositoryError> for GitHttpError {
    fn from(error: RepositoryError) -> Self {
        match error {
            RepositoryError::RepositoryMissing(path) => Self::not_found(format!(
                "Repository storage path is missing: {}.",
                path.display()
            )),
            other => Self::internal(format!("Git repository error: {other}")),
        }
    }
}

impl From<sqlx::Error> for GitHttpError {
    fn from(error: sqlx::Error) -> Self {
        Self::internal(format!("Database operation failed: {error}"))
    }
}

