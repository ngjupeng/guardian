use crate::auth::Credentials;
use crate::state::AppState;
use crate::storage::DeltaObject;

use super::common::{ServiceError, ServiceResult, verify_request_auth};

#[derive(Debug, Clone)]
pub struct GetDeltaHeadParams {
    pub account_id: String,
    pub credentials: Credentials,
}

#[derive(Debug, Clone)]
pub struct GetDeltaHeadResult {
    pub delta: DeltaObject,
}

/// Get the latest delta (head) for an account
pub async fn get_delta_head(
    state: &AppState,
    params: GetDeltaHeadParams,
) -> ServiceResult<GetDeltaHeadResult> {
    // Verify account exists
    let account_metadata = state
        .metadata
        .get(&params.account_id)
        .await
        .map_err(|e| ServiceError::new(format!("Failed to check account: {e}")))?
        .ok_or_else(|| ServiceError::new(format!("Account '{}' not found", params.account_id)))?;

    // Verify authentication and authorization
    verify_request_auth(
        &account_metadata.auth,
        &params.account_id,
        &params.credentials,
    )?;

    // Get the storage backend for this account
    let storage_backend = state
        .storage
        .get(&account_metadata.storage_type)
        .map_err(ServiceError::new)?;

    // Fetch all deltas and find the latest non-discarded one
    let all_deltas = storage_backend
        .pull_deltas_after(&params.account_id, 0)
        .await
        .map_err(|e| ServiceError::new(format!("Failed to fetch deltas: {e}")))?;

    // Filter out discarded deltas and get the latest
    let delta = all_deltas
        .into_iter()
        .filter(|d| d.discarded_at.is_none())
        .max_by_key(|d| d.nonce)
        .ok_or_else(|| {
            ServiceError::new(format!(
                "No valid deltas found for account '{}'",
                params.account_id
            ))
        })?;

    Ok(GetDeltaHeadResult { delta })
}
