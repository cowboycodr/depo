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
    let repo = create_repo_with_commit(&server, "Initial commit", "README.md", "# Depo\n").await;
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

    let lands = db::list_lands(&server.state.db, "kian/depo", 10)
        .await
        .unwrap();
    assert_eq!(lands.len(), 1);
    assert_eq!(lands[0].actor, "local");
    assert_eq!(lands[0].source, "git-http");
    assert_eq!(lands[0].short_ref, "main");
    assert_eq!(lands[0].kind, "branch_updated");
    assert_eq!(lands[0].head_title.as_deref(), Some("Client update"));
    assert_eq!(lands[0].commit_count, 1);
    assert_eq!(lands[0].new_sha, commits[0].sha.as_str());
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

    let lands = db::list_lands(&server.state.db, "kian/depo", 10)
        .await
        .unwrap();
    assert_eq!(lands.len(), 1);
    assert_eq!(lands[0].short_ref, "main");
    assert_eq!(lands[0].kind, "branch_created");
    assert_eq!(lands[0].head_title.as_deref(), Some("Initial push"));
    assert_eq!(lands[0].commit_count, 1);
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
