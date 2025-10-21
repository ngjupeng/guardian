use crate::auth::Credentials;
use crate::error::{PsmError, Result};
use crate::state::AppState;
use crate::storage::DeltaObject;
use crate::services::resolve_account;

#[derive(Debug, Clone)]
pub struct GetDeltaHeadParams {
    pub account_id: String,
    pub credentials: Credentials,
}

#[derive(Debug, Clone)]
pub struct GetDeltaHeadResult {
    pub delta: DeltaObject,
}

pub async fn get_delta_head(
    state: &AppState,
    params: GetDeltaHeadParams,
) -> Result<GetDeltaHeadResult> {
    let account_id = params.account_id.clone();
    let resolved = resolve_account(state, &account_id, &params.credentials).await?;

    if let Some(nonce) = resolved
        .backend
        .get_delta_head(&account_id)
        .await
        .map_err(|e| PsmError::StorageError(format!("Failed to fetch head: {e}")))? {
        let delta = resolved
            .backend
            .pull_delta(&account_id, nonce)
            .await
            .map_err(|_e| PsmError::DeltaNotFound { account_id, nonce })?;
        return Ok(GetDeltaHeadResult { delta });
    }

    Err(PsmError::DeltaNotFound { account_id, nonce: 0 })
}
