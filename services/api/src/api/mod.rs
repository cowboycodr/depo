pub mod dto;
pub mod errors;
pub mod handlers;

pub use errors::ApiError;

#[cfg(test)]
mod tests {
    use axum::{
        body::{Body, to_bytes},
        http::{Method, Request},
    };
    use serde_json::{Value, json};
    use tempfile::TempDir;
    use tower::ServiceExt;

    use crate::{AppState, AuthMode, config, migrate, router};

    async fn test_app() -> (axum::Router, TempDir) {
        let temp = tempfile::tempdir().unwrap();
        let database_url = format!("sqlite://{}", temp.path().join("depo.db").display());
        let db = config::connect_database(&database_url).await.unwrap();
        migrate(&db).await.unwrap();
        let state = AppState {
            db,
            storage: depo_core::git::StorageRoot::new(temp.path().join("repos")).unwrap(),
            git: depo_core::git::GitCommand::default(),
            inline_blob_limit: 1024 * 1024,
            git_http_body_limit: 64 * 1024 * 1024,
            auth_mode: AuthMode::Local,
        };
        (router(state), temp)
    }

    #[tokio::test]
    async fn creates_repo_commit_and_view_returns_actual_code() {
        let (app, _temp) = test_app().await;

        let response = app
            .clone()
            .oneshot(json_request(
                Method::POST,
                "/api/v1/repos",
                json!({
                    "owner": "kian",
                    "name": "depo",
                    "defaultBranch": "main"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::CREATED);

        let response = app
            .clone()
            .oneshot(json_request(
                Method::POST,
                "/api/v1/repos/kian/depo/commits",
                json!({
                    "targetBranch": "main",
                    "message": "Initial commit",
                    "author": { "name": "Kian", "email": "kian@example.com" },
                    "changes": [
                        {
                            "type": "upsertText",
                            "path": "README.md",
                            "content": "# Depo\n"
                        },
                        {
                            "type": "upsertText",
                            "path": "src/main.rs",
                            "content": "fn main() {}\n"
                        }
                    ]
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/repos/kian/depo/view?ref=main&path=README.md")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        assert_eq!(status, axum::http::StatusCode::OK, "{}", String::from_utf8_lossy(&body));
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["activeFile"]["content"], "# Depo\n");
        assert_eq!(json["tree"]["nodes"][0]["path"], "README.md");
        assert!(
            json["tree"]["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|node| node["path"] == "src/main.rs")
        );
        assert_eq!(json["recentCommits"][0]["title"], "Initial commit");
    }

    #[tokio::test]
    async fn commit_detail_and_diff_return_file_contents() {
        let (app, _temp) = test_app().await;

        let response = app
            .clone()
            .oneshot(json_request(
                Method::POST,
                "/api/v1/repos",
                json!({
                    "owner": "kian",
                    "name": "depo",
                    "defaultBranch": "main"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::CREATED);

        let response = app
            .clone()
            .oneshot(json_request(
                Method::POST,
                "/api/v1/repos/kian/depo/commits",
                json!({
                    "targetBranch": "main",
                    "message": "Initial commit",
                    "author": { "name": "Kian", "email": "kian@example.com" },
                    "changes": [
                        {
                            "type": "upsertText",
                            "path": "README.md",
                            "content": "# Depo\n"
                        }
                    ]
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let first: Value = serde_json::from_slice(&body).unwrap();
        let first_sha = first["commit"]["sha"].as_str().unwrap();

        let response = app
            .clone()
            .oneshot(json_request(
                Method::POST,
                "/api/v1/repos/kian/depo/commits",
                json!({
                    "targetBranch": "main",
                    "expectedHeadSha": first_sha,
                    "message": "Expand README",
                    "author": { "name": "Kian", "email": "kian@example.com" },
                    "changes": [
                        {
                            "type": "upsertText",
                            "path": "README.md",
                            "content": "# Depo\n\nReal code hosting.\n"
                        },
                        {
                            "type": "upsertText",
                            "path": "src/main.rs",
                            "content": "fn main() {}\n"
                        }
                    ]
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let second: Value = serde_json::from_slice(&body).unwrap();
        let second_sha = second["commit"]["sha"].as_str().unwrap();

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri(format!("/api/v1/repos/kian/depo/commits/{second_sha}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        assert_eq!(status, axum::http::StatusCode::OK, "{}", String::from_utf8_lossy(&body));
        let detail: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(detail["commit"]["title"], "Expand README");
        assert_eq!(detail["commit"]["parents"][0], first_sha);
        assert_eq!(detail["diff"]["baseSha"], first_sha);
        assert_eq!(detail["diff"]["headSha"], second_sha);
        assert_eq!(detail["diff"]["files"][0]["status"], "modified");
        assert_eq!(detail["diff"]["files"][0]["oldFile"]["content"], "# Depo\n");
        assert_eq!(
            detail["diff"]["files"][0]["newFile"]["content"],
            "# Depo\n\nReal code hosting.\n"
        );

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri(format!(
                        "/api/v1/repos/kian/depo/diff?base={first_sha}&head={second_sha}&path=src/main.rs"
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        assert_eq!(status, axum::http::StatusCode::OK, "{}", String::from_utf8_lossy(&body));
        let diff: Value = serde_json::from_slice(&body).unwrap();
        let selected = diff["diff"]["files"]
            .as_array()
            .unwrap()
            .iter()
            .find(|file| file["path"] == "src/main.rs")
            .unwrap();
        assert_eq!(selected["newFile"]["content"], "fn main() {}\n");
        assert_eq!(selected["newFile"]["language"], "rust");
    }

    #[tokio::test]
    async fn commit_detail_returns_not_found_for_missing_commit() {
        let (app, _temp) = test_app().await;

        let response = app
            .clone()
            .oneshot(json_request(
                Method::POST,
                "/api/v1/repos",
                json!({
                    "owner": "kian",
                    "name": "depo",
                    "defaultBranch": "main"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::CREATED);

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/repos/kian/depo/commits/1111111111111111111111111111111111111111")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        assert_eq!(
            status,
            axum::http::StatusCode::NOT_FOUND,
            "{}",
            String::from_utf8_lossy(&body)
        );
        let error: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(error["error"]["code"], "commit_not_found");
    }

    fn json_request(method: Method, uri: &str, body: Value) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    }
}
