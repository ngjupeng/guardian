use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PsmError {
    AccountNotFound(String),
    AccountAlreadyExists(String),
    AuthenticationFailed(String),
    AuthorizationFailed(String),
    StorageError(String),
    NetworkError(String),
    InvalidInput(String),
    ConflictPendingDelta,
    DeltaNotFound { account_id: String, nonce: u64 },
    StateNotFound(String),
    CommitmentMismatch { expected: String, actual: String },
    InvalidAccountId(String),
    InvalidDelta(String),
    ConfigurationError(String),
}

impl PsmError {
    pub fn http_status(&self) -> StatusCode {
        match self {
            PsmError::AccountNotFound(_) => StatusCode::NOT_FOUND,
            PsmError::DeltaNotFound { .. } => StatusCode::NOT_FOUND,
            PsmError::StateNotFound(_) => StatusCode::NOT_FOUND,
            PsmError::AccountAlreadyExists(_) => StatusCode::CONFLICT,
            PsmError::ConflictPendingDelta => StatusCode::CONFLICT,
            PsmError::AuthenticationFailed(_) => StatusCode::UNAUTHORIZED,
            PsmError::AuthorizationFailed(_) => StatusCode::FORBIDDEN,
            PsmError::InvalidInput(_) => StatusCode::BAD_REQUEST,
            PsmError::InvalidAccountId(_) => StatusCode::BAD_REQUEST,
            PsmError::InvalidDelta(_) => StatusCode::BAD_REQUEST,
            PsmError::CommitmentMismatch { .. } => StatusCode::BAD_REQUEST,
            PsmError::StorageError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            PsmError::NetworkError(_) => StatusCode::BAD_GATEWAY,
            PsmError::ConfigurationError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub fn grpc_status(&self) -> tonic::Code {
        match self {
            PsmError::AccountNotFound(_) => tonic::Code::NotFound,
            PsmError::DeltaNotFound { .. } => tonic::Code::NotFound,
            PsmError::StateNotFound(_) => tonic::Code::NotFound,
            PsmError::AccountAlreadyExists(_) => tonic::Code::AlreadyExists,
            PsmError::ConflictPendingDelta => tonic::Code::FailedPrecondition,
            PsmError::AuthenticationFailed(_) => tonic::Code::Unauthenticated,
            PsmError::AuthorizationFailed(_) => tonic::Code::PermissionDenied,
            PsmError::InvalidInput(_) => tonic::Code::InvalidArgument,
            PsmError::InvalidAccountId(_) => tonic::Code::InvalidArgument,
            PsmError::InvalidDelta(_) => tonic::Code::InvalidArgument,
            PsmError::CommitmentMismatch { .. } => tonic::Code::InvalidArgument,
            PsmError::StorageError(_) => tonic::Code::Internal,
            PsmError::NetworkError(_) => tonic::Code::Unavailable,
            PsmError::ConfigurationError(_) => tonic::Code::Internal,
        }
    }
}

impl fmt::Display for PsmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PsmError::AccountNotFound(id) => write!(f, "Account '{}' not found", id),
            PsmError::AccountAlreadyExists(id) => write!(f, "Account '{}' already exists", id),
            PsmError::AuthenticationFailed(msg) => write!(f, "Authentication failed: {}", msg),
            PsmError::AuthorizationFailed(msg) => write!(f, "Authorization failed: {}", msg),
            PsmError::StorageError(msg) => write!(f, "Storage error: {}", msg),
            PsmError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            PsmError::InvalidInput(msg) => write!(f, "Invalid input: {}", msg),
            PsmError::ConflictPendingDelta => {
                write!(f, "Cannot push new delta: there is already a non-canonical delta pending")
            }
            PsmError::DeltaNotFound { account_id, nonce } => {
                write!(f, "Delta not found for account '{}' at nonce {}", account_id, nonce)
            }
            PsmError::StateNotFound(id) => write!(f, "State not found for account '{}'", id),
            PsmError::CommitmentMismatch { expected, actual } => {
                write!(f, "Commitment mismatch: expected {}, got {}", expected, actual)
            }
            PsmError::InvalidAccountId(msg) => write!(f, "Invalid account ID: {}", msg),
            PsmError::InvalidDelta(msg) => write!(f, "Invalid delta: {}", msg),
            PsmError::ConfigurationError(msg) => write!(f, "Configuration error: {}", msg),
        }
    }
}

impl std::error::Error for PsmError {}

impl From<String> for PsmError {
    fn from(s: String) -> Self {
        PsmError::InvalidInput(s)
    }
}

impl From<&str> for PsmError {
    fn from(s: &str) -> Self {
        PsmError::InvalidInput(s.to_string())
    }
}

#[derive(Serialize)]
struct ErrorResponse {
    success: bool,
    error: String,
}

impl IntoResponse for PsmError {
    fn into_response(self) -> Response {
        let status = self.http_status();
        let body = Json(ErrorResponse {
            success: false,
            error: self.to_string(),
        });
        (status, body).into_response()
    }
}

impl From<PsmError> for tonic::Status {
    fn from(err: PsmError) -> Self {
        tonic::Status::new(err.grpc_status(), err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, PsmError>;
