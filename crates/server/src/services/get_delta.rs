use crate::auth::Credentials;
use crate::error::{PsmError, Result};
use crate::state::AppState;
use crate::storage::DeltaObject;

#[derive(Debug, Clone)]
pub struct GetDeltaParams {
    pub account_id: String,
    pub nonce: u64,
    pub credentials: Credentials,
}

#[derive(Debug, Clone)]
pub struct GetDeltaResult {
    pub delta: DeltaObject,
}

/// Get a specific delta
pub async fn get_delta(state: &AppState, params: GetDeltaParams) -> Result<GetDeltaResult> {
    let account_metadata = state
        .metadata
        .get(&params.account_id)
        .await
        .map_err(|e| PsmError::StorageError(format!("Failed to check account: {e}")))?
        .ok_or_else(|| PsmError::AccountNotFound(params.account_id.clone()))?;

    account_metadata
        .auth
        .verify(&params.account_id, &params.credentials)
        .map_err(PsmError::AuthenticationFailed)?;

    let storage_backend = state
        .storage
        .get(&account_metadata.storage_type)
        .map_err(PsmError::ConfigurationError)?;

    let delta = storage_backend
        .pull_delta(&params.account_id, params.nonce)
        .await
        .map_err(|_e| PsmError::DeltaNotFound {
            account_id: params.account_id.clone(),
            nonce: params.nonce,
        })?;

    Ok(GetDeltaResult { delta })
}
