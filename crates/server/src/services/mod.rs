use crate::state::AppState;
use crate::storage::{AccountState, DeltaObject};

pub type ServiceResult<T> = Result<T, ServiceError>;

#[derive(Debug, Clone)]
pub struct ServiceError {
    pub message: String,
}

impl ServiceError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// Configure a new account
pub async fn configure_account(
    _state: &AppState,
    _account_id: String,
    _initial_state: serde_json::Value,
    _storage_type: String,
    _cosigner_pubkeys: Vec<String>,
) -> ServiceResult<()> {
    // TODO: Implement
    Ok(())
}

/// Push a delta
pub async fn push_delta(_state: &AppState, delta: DeltaObject) -> ServiceResult<DeltaObject> {
    // TODO: Implement
    Ok(delta)
}

/// Get a specific delta
pub async fn get_delta(
    _state: &AppState,
    _account_id: &str,
    _nonce: u64,
) -> ServiceResult<DeltaObject> {
    // TODO: Implement
    Ok(DeltaObject::default())
}

/// Get the latest delta (head) for an account
pub async fn get_delta_head(_state: &AppState, _account_id: &str) -> ServiceResult<DeltaObject> {
    // TODO: Implement
    Ok(DeltaObject::default())
}

/// Get the latest nonce for an account (returns None if no deltas exist)
pub async fn get_latest_nonce(_state: &AppState, _account_id: &str) -> ServiceResult<Option<u64>> {
    // TODO: Implement
    Ok(None)
}

/// Get account state
pub async fn get_state(_state: &AppState, _account_id: &str) -> ServiceResult<AccountState> {
    // TODO: Implement
    Ok(AccountState::default())
}
