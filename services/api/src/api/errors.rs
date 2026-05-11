use axum::{
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use depo_core::git::{IdError, RepositoryError};
use serde::Serialize;
use serde_json::{Value, json};

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
    pub details: Value,
}

impl ApiError {
    pub fn bad_request(code: &'static str, message: impl Into<String>, details: Value) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code,
            message: message.into(),
            details,
        }
    }

    pub fn not_found(code: &'static str, message: impl Into<String>, details: Value) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code,
            message: message.into(),
            details,
        }
    }

    pub fn conflict(code: &'static str, message: impl Into<String>, details: Value) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            code,
            message: message.into(),
            details,
        }
    }

    pub fn internal(code: &'static str, message: impl Into<String>, details: Value) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code,
            message: message.into(),
            details,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = ErrorEnvelope {
            error: ErrorBody {
                code: self.code,
                message: self.message,
                details: self.details,
            },
        };
        (self.status, Json(body)).into_response()
    }
}

#[derive(Debug, Serialize)]
pub struct ErrorEnvelope {
    pub error: ErrorBody,
}

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub code: &'static str,
    pub message: String,
    pub details: Value,
}

impl From<IdError> for ApiError {
    fn from(error: IdError) -> Self {
        Self::bad_request("invalid_input", error.to_string(), json!({}))
    }
}

impl From<RepositoryError> for ApiError {
    fn from(error: RepositoryError) -> Self {
        match error {
            RepositoryError::Id(error) => ApiError::from(error),
            RepositoryError::RepositoryExists(path) => Self::conflict(
                "repo_storage_exists",
                "Repository storage path already exists.",
                json!({ "path": path.display().to_string() }),
            ),
            RepositoryError::BranchMissing(branch) => Self::not_found(
                "ref_not_found",
                format!("Branch {branch} does not exist."),
                json!({ "branch": branch }),
            ),
            RepositoryError::CommitMissing(sha) => Self::not_found(
                "commit_not_found",
                format!("Commit {sha} does not exist."),
                json!({ "sha": sha }),
            ),
            RepositoryError::HeadMismatch { expected, actual } => Self::conflict(
                "head_mismatch",
                "Branch head did not match expectedHeadSha.",
                json!({ "expected": expected, "actual": actual }),
            ),
            RepositoryError::EmptyCommit
            | RepositoryError::EmptyCommitMessage
            | RepositoryError::UnsupportedFileMode(_) => {
                Self::bad_request("invalid_commit", error.to_string(), json!({}))
            }
            RepositoryError::PathNotFound(path) => Self::not_found(
                "path_not_found",
                "Repository path does not exist.",
                json!({ "path": path }),
            ),
            RepositoryError::PathNotFile(path) => Self::bad_request(
                "path_not_file",
                "Repository path is not a file.",
                json!({ "path": path }),
            ),
            RepositoryError::RepositoryMissing(path) => Self::not_found(
                "repo_storage_missing",
                "Repository storage path is missing.",
                json!({ "path": path.display().to_string() }),
            ),
            other => Self::internal(
                "git_error",
                "Git operation failed.",
                json!({
                    "reason": other.to_string()
                }),
            ),
        }
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(error: sqlx::Error) -> Self {
        Self::internal(
            "database_error",
            "Database operation failed.",
            json!({
                "reason": error.to_string()
            }),
        )
    }
}
