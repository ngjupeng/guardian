use std::sync::Arc;
use crate::auth::Credentials;
use crate::error::{PsmError, Result};
use crate::state::AppState;
use crate::storage::{AccountMetadata, StorageBackend};

mod canonicalization;
mod configure_account;
mod get_delta;
mod get_delta_head;
mod get_delta_since;
mod get_state;
mod push_delta;

pub use canonicalization::{process_canonicalizations_now, start_canonicalization_worker};
pub use configure_account::{configure_account, ConfigureAccountParams, ConfigureAccountResult};
pub use get_delta::{get_delta, GetDeltaParams, GetDeltaResult};
pub use get_delta_head::{get_delta_head, GetDeltaHeadParams, GetDeltaHeadResult};
pub use get_delta_since::{get_delta_since, GetDeltaSinceParams, GetDeltaSinceResult};
pub use get_state::{get_state, GetStateParams, GetStateResult};
pub use push_delta::{push_delta, PushDeltaParams, PushDeltaResult};

#[derive(Clone)]
pub struct ResolvedAccount {
    pub metadata: AccountMetadata,
    pub backend: Arc<dyn StorageBackend>,
}

pub async fn resolve_account(
    state: &AppState,
    account_id: &str,
    creds: &Credentials,
) -> Result<ResolvedAccount> {
    let metadata = state
        .metadata
        .get(account_id)
        .await
        .map_err(|e| PsmError::StorageError(format!("Failed to check account: {e}")))?
        .ok_or_else(|| PsmError::AccountNotFound(account_id.to_string()))?;

    metadata
        .auth
        .verify(account_id, creds)
        .map_err(PsmError::AuthenticationFailed)?;

    let backend = state
        .storage
        .get(&metadata.storage_type)
        .map_err(PsmError::ConfigurationError)?;

    Ok(ResolvedAccount { metadata, backend })
}
