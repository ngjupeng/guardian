use thiserror::Error;

pub type ClientResult<T> = Result<T, ClientError>;

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("gRPC transport error: {0}")]
    Transport(#[from] tonic::transport::Error),

    #[error("gRPC status error: {0}")]
    Status(#[from] tonic::Status),

    #[error("Server returned error: {0}")]
    ServerError(String),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Invalid response: {0}")]
    InvalidResponse(String),
}
