use crate::auth::Credentials;
use crate::state::AppState;
use crate::storage::DeltaObject;

use super::common::{ServiceError, ServiceResult, verify_request_auth};

#[derive(Debug, Clone)]
pub struct PushDeltaParams {
    pub delta: DeltaObject,
    pub credentials: Credentials,
}

#[derive(Debug, Clone)]
pub struct PushDeltaResult {
    pub delta: DeltaObject,
}

/// Push a delta
pub async fn push_delta(
    state: &AppState,
    params: PushDeltaParams,
) -> ServiceResult<PushDeltaResult> {
    // Verify account exists
    let account_metadata = state
        .metadata
        .get(&params.delta.account_id)
        .await
        .map_err(|e| ServiceError::new(format!("Failed to check account: {e}")))?
        .ok_or_else(|| {
            ServiceError::new(format!("Account '{}' not found", params.delta.account_id))
        })?;

    // Verify authentication and authorization
    verify_request_auth(
        &account_metadata.auth,
        &params.delta.account_id,
        &params.credentials,
    )?;

    // Get the storage backend for this account
    let storage_backend = state
        .storage
        .get(&account_metadata.storage_type)
        .map_err(ServiceError::new)?;

    // Verify delta payload is valid by attempting to deserialize as AccountDelta
    {
        let client = state.network_client.lock().await;
        client
            .verify_delta(&params.delta.delta_payload)
            .map_err(|e| ServiceError::new(format!("Invalid delta payload: {e}")))?;
    }

    // TODO: verify nonce is greater than the max nonce for the account
    // TODO: Verify prev_commitment matches current state commitment

    // Submit delta to storage
    storage_backend
        .submit_delta(&params.delta)
        .await
        .map_err(|e| ServiceError::new(format!("Failed to submit delta: {e}")))?;


    // TODO: Verify new commitment vs on-chain commitment in time window.
    
    // TODO: Create ack signature
    Ok(PushDeltaResult {
        delta: params.delta,
    })
}
