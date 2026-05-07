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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use axum::{
        body::{Body, to_bytes},
        http::{Method, Request, StatusCode, header},
    };
    use depo_core::git::{
        BareRepository, BranchName, CommitAuthor, CommitChange, CommitRequest, GitCommand,
        GitCommandRequest, RepoFilePath, RepoId, StorageRoot, ValidatedRef,
    };
    use tempfile::TempDir;
    use tokio::{net::TcpListener, task::JoinHandle};
    use tower::ServiceExt;

    use crate::{AppState, AuthMode, config, db, migrate, router};

    struct TestServer {
        base_url: String,
        state: AppState,
        _temp: TempDir,
        task: JoinHandle<()>,
    }

    impl Drop for TestServer {
        fn drop(&mut self) {
            self.task.abort();
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn clone_fetch_and_push_work_over_authenticated_smart_http() {
        let server = spawn_server().await;
        let repo =
            create_repo_with_commit(&server, "Initial commit", "README.md", "# Depo\n").await;
        let work = tempfile::tempdir().unwrap();
        let clone_path = work.path().join("clone");
        let remote = format!("{}/kian/depo.git", authed_base_url(&server));

        run_git(vec!["clone".to_owned(), remote, path_arg(&clone_path)]);
        assert_eq!(
            std::fs::read_to_string(clone_path.join("README.md")).unwrap(),
            "# Depo\n"
        );

        let server_commit = repo
            .create_commit(CommitRequest {
                target_branch: BranchName::parse("main").unwrap(),
                expected_head_sha: None,
                message: "Server-side update".to_owned(),
                author: test_author(),
                changes: vec![CommitChange::Upsert {
                    path: RepoFilePath::parse_file("server.txt").unwrap(),
                    content: b"from server\n".to_vec(),
                    mode: "100644".to_owned(),
                }],
            })
            .unwrap();

        run_git(vec![
            "-C".to_owned(),
            path_arg(&clone_path),
            "fetch".to_owned(),
            "origin".to_owned(),
        ]);
        let fetched = run_git(vec![
            "-C".to_owned(),
            path_arg(&clone_path),
            "rev-parse".to_owned(),
            "origin/main".to_owned(),
        ]);
        assert_eq!(
            fetched.stdout_string().unwrap().trim(),
            server_commit.sha.as_str()
        );

        run_git(vec![
            "-C".to_owned(),
            path_arg(&clone_path),
            "merge".to_owned(),
            "--ff-only".to_owned(),
            "origin/main".to_owned(),
        ]);
        run_git(vec![
            "-C".to_owned(),
            path_arg(&clone_path),
            "config".to_owned(),
            "user.name".to_owned(),
            "Kian".to_owned(),
        ]);
        run_git(vec![
            "-C".to_owned(),
            path_arg(&clone_path),
            "config".to_owned(),
            "user.email".to_owned(),
            "kian@example.com".to_owned(),
        ]);
        std::fs::write(clone_path.join("client.txt"), "from client\n").unwrap();
        run_git(vec![
            "-C".to_owned(),
            path_arg(&clone_path),
            "add".to_owned(),
            "client.txt".to_owned(),
        ]);
        run_git(vec![
            "-C".to_owned(),
            path_arg(&clone_path),
            "commit".to_owned(),
            "-m".to_owned(),
            "Client update".to_owned(),
        ]);
        run_git(vec![
            "-C".to_owned(),
            path_arg(&clone_path),
            "push".to_owned(),
            "origin".to_owned(),
            "main".to_owned(),
        ]);

        let blob = repo
            .read_blob(
                &ValidatedRef::Branch(BranchName::parse("main").unwrap()),
                &RepoFilePath::parse_file("client.txt").unwrap(),
                1024 * 1024,
            )
            .unwrap();
        assert_eq!(blob.content.as_deref(), Some("from client\n"));
        let commits = repo
            .recent_commits(&ValidatedRef::Branch(BranchName::parse("main").unwrap()), 1)
            .unwrap();
        assert_eq!(commits[0].title, "Client update");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn push_to_empty_repository_creates_default_branch() {
        let server = spawn_server().await;
        let repo = create_empty_repo(&server).await;
        let work = tempfile::tempdir().unwrap();
        let local_path = work.path().join("local");

        run_git(vec![
            "init".to_owned(),
            "-b".to_owned(),
            "main".to_owned(),
            path_arg(&local_path),
        ]);
        run_git(vec![
            "-C".to_owned(),
            path_arg(&local_path),
            "config".to_owned(),
            "user.name".to_owned(),
            "Kian".to_owned(),
        ]);
        run_git(vec![
            "-C".to_owned(),
            path_arg(&local_path),
            "config".to_owned(),
            "user.email".to_owned(),
            "kian@example.com".to_owned(),
        ]);
        std::fs::write(local_path.join("README.md"), "# Empty no more\n").unwrap();
        run_git(vec![
            "-C".to_owned(),
            path_arg(&local_path),
            "add".to_owned(),
            "README.md".to_owned(),
        ]);
        run_git(vec![
            "-C".to_owned(),
            path_arg(&local_path),
            "commit".to_owned(),
            "-m".to_owned(),
            "Initial push".to_owned(),
        ]);
        run_git(vec![
            "-C".to_owned(),
            path_arg(&local_path),
            "remote".to_owned(),
            "add".to_owned(),
            "origin".to_owned(),
            format!("{}/kian/depo.git", authed_base_url(&server)),
        ]);
        run_git(vec![
            "-C".to_owned(),
            path_arg(&local_path),
            "push".to_owned(),
            "-u".to_owned(),
            "origin".to_owned(),
            "main".to_owned(),
        ]);

        let blob = repo
            .read_blob(
                &ValidatedRef::Branch(BranchName::parse("main").unwrap()),
                &RepoFilePath::parse_file("README.md").unwrap(),
                1024 * 1024,
            )
            .unwrap();
        assert_eq!(blob.content.as_deref(), Some("# Empty no more\n"));
    }

    #[tokio::test]
    async fn git_smart_http_requires_credentials() {
        let temp = tempfile::tempdir().unwrap();
        let database_url = format!("sqlite://{}", temp.path().join("depo.db").display());
        let db = config::connect_database(&database_url).await.unwrap();
        migrate(&db).await.unwrap();
        let state = AppState {
            db,
            storage: StorageRoot::new(temp.path().join("repos")).unwrap(),
            git: GitCommand::default(),
            inline_blob_limit: 1024 * 1024,
            git_http_body_limit: 64 * 1024 * 1024,
            auth_mode: AuthMode::Local,
        };
        let app = router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/kian/depo.git/info/refs?service=git-upload-pack")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            response.headers().get(header::WWW_AUTHENTICATE).unwrap(),
            "Basic realm=\"Depo Git\""
        );
        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        assert!(String::from_utf8_lossy(&body).contains("authentication is required"));
    }

    async fn spawn_server() -> TestServer {
        let temp = tempfile::tempdir().unwrap();
        let database_url = format!("sqlite://{}", temp.path().join("depo.db").display());
        let db = config::connect_database(&database_url).await.unwrap();
        migrate(&db).await.unwrap();
        let state = AppState {
            db,
            storage: StorageRoot::new(temp.path().join("repos")).unwrap(),
            git: GitCommand::default(),
            inline_blob_limit: 1024 * 1024,
            git_http_body_limit: 64 * 1024 * 1024,
            auth_mode: AuthMode::Local,
        };
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = router(state.clone());
        let task = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        TestServer {
            base_url: format!("http://{addr}"),
            state,
            _temp: temp,
            task,
        }
    }

    async fn create_repo_with_commit(
        server: &TestServer,
        message: &str,
        path: &str,
        content: &str,
    ) -> BareRepository {
        let repo = create_empty_repo(server).await;
        repo.create_commit(CommitRequest {
            target_branch: BranchName::parse("main").unwrap(),
            expected_head_sha: None,
            message: message.to_owned(),
            author: test_author(),
            changes: vec![CommitChange::Upsert {
                path: RepoFilePath::parse_file(path).unwrap(),
                content: content.as_bytes().to_vec(),
                mode: "100644".to_owned(),
            }],
        })
        .unwrap();
        repo
    }

    async fn create_empty_repo(server: &TestServer) -> BareRepository {
        let id = RepoId::parse("kian", "depo").unwrap();
        let repo = BareRepository::create(
            &server.state.storage,
            id.clone(),
            BranchName::parse("main").unwrap(),
            server.state.git.clone(),
        )
        .unwrap();
        db::insert_repository(
            &server.state.db,
            &id.as_full_name(),
            id.owner().as_str(),
            id.name().as_str(),
            "main",
            repo.path().to_str().unwrap(),
        )
        .await
        .unwrap();
        repo
    }

    fn authed_base_url(server: &TestServer) -> String {
        server.base_url.replace("http://", "http://git:local@")
    }

    fn test_author() -> CommitAuthor {
        CommitAuthor {
            name: "Kian".to_owned(),
            email: "kian@example.com".to_owned(),
        }
    }

    fn run_git(args: Vec<String>) -> depo_core::git::GitCommandOutput {
        GitCommand::default()
            .run(
                GitCommandRequest::new(args)
                    .env("GIT_TERMINAL_PROMPT", "0")
                    .timeout(Duration::from_secs(60)),
            )
            .unwrap()
    }

    fn path_arg(path: &std::path::Path) -> String {
        path.to_str().unwrap().to_owned()
    }
}
