use crate::metadata::AccountMetadata;
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
    state: &AppState,
    account_id: String,
    initial_state: serde_json::Value,
    storage_type: String,
    cosigner_pubkeys: Vec<String>,
) -> ServiceResult<()> {
    // Check if account already exists
    let mut metadata = state.metadata.lock().await;
    let existing = metadata.get_account(&account_id).await
        .map_err(|e| ServiceError::new(format!("Failed to check existing account: {}", e)))?;

    if existing.is_some() {
        return Err(ServiceError::new(format!("Account '{}' already exists", account_id)));
    }

    // Create initial account state
    let now = chrono::Utc::now().to_rfc3339();
    let account_state = AccountState {
        account_id: account_id.clone(),
        state_json: initial_state,
        commitment: String::new(), // TODO: calculate commitment + validate vs on-chain commitment.
        created_at: now.clone(),
        updated_at: now,
    };

    // Submit initial state to storage
    state.storage.submit_state(&account_state).await
        .map_err(|e| ServiceError::new(format!("Failed to submit initial state: {}", e)))?;

    // Create and store metadata
    let metadata_entry = AccountMetadata {
        account_id: account_id.clone(),
        storage_type,
        cosigner_pubkeys,
        created_at: account_state.created_at.clone(),
        updated_at: account_state.updated_at.clone(),
    };

    metadata.set_account(metadata_entry).await
        .map_err(|e| ServiceError::new(format!("Failed to store metadata: {}", e)))?;

    Ok(())
}

/// Push a delta
pub async fn push_delta(state: &AppState, delta: DeltaObject) -> ServiceResult<DeltaObject> {
    // Verify account exists
    let metadata = state.metadata.lock().await;
    metadata.get_account(&delta.account_id).await
        .map_err(|e| ServiceError::new(format!("Failed to check account: {}", e)))?
        .ok_or_else(|| ServiceError::new(format!("Account '{}' not found", delta.account_id)))?;
    drop(metadata);

    // TODO: Verify publisher_signature
    // TODO: Verify prev_commitment matches current state commitment
    // TODO: Verify delta_hash vs on-chain commitment
    // TODO: Verify nonce is valid (could check against on-chain or validate it doesn't already exist)

    // Submit delta to storage
    state.storage.submit_delta(&delta).await
        .map_err(|e| ServiceError::new(format!("Failed to submit delta: {}", e)))?;

    // TODO: Create ack signature
    Ok(delta)
}

/// Get a specific delta
pub async fn get_delta(
    state: &AppState,
    account_id: &str,
    nonce: u64,
) -> ServiceResult<DeltaObject> {
    // Verify account exists
    let metadata = state.metadata.lock().await;
    metadata.get_account(account_id).await
        .map_err(|e| ServiceError::new(format!("Failed to check account: {}", e)))?
        .ok_or_else(|| ServiceError::new(format!("Account '{}' not found", account_id)))?;
    drop(metadata);

    // Fetch delta from storage
    let delta = state.storage.pull_delta(account_id, nonce).await
        .map_err(|e| ServiceError::new(format!("Failed to fetch delta: {}", e)))?;

    Ok(delta)
}

/// Get the latest delta (head) for an account
pub async fn get_delta_head(state: &AppState, account_id: &str) -> ServiceResult<DeltaObject> {
    // Verify account exists
    let metadata = state.metadata.lock().await;
    metadata.get_account(account_id).await
        .map_err(|e| ServiceError::new(format!("Failed to check account: {}", e)))?
        .ok_or_else(|| ServiceError::new(format!("Account '{}' not found", account_id)))?;
    drop(metadata);

    // Get list of deltas
    let delta_files = state.storage.list_deltas(account_id).await
        .map_err(|e| ServiceError::new(format!("Failed to list deltas: {}", e)))?;

    if delta_files.is_empty() {
        return Err(ServiceError::new(format!("No deltas found for account '{}'", account_id)));
    }

    // Parse nonces from filenames and find the maximum
    let mut max_nonce: Option<u64> = None;
    for filename in &delta_files {
        if let Some(nonce_str) = filename.strip_suffix(".json") {
            if let Ok(nonce) = nonce_str.parse::<u64>() {
                max_nonce = Some(max_nonce.map_or(nonce, |current| current.max(nonce)));
            }
        }
    }

    let latest_nonce = max_nonce
        .ok_or_else(|| ServiceError::new("Failed to parse nonces from delta files".to_string()))?;

    // Fetch the latest delta
    let delta = state.storage.pull_delta(account_id, latest_nonce).await
        .map_err(|e| ServiceError::new(format!("Failed to fetch latest delta: {}", e)))?;

    Ok(delta)
}

/// Get the latest nonce for an account (returns None if no deltas exist)
pub async fn get_latest_nonce(state: &AppState, account_id: &str) -> ServiceResult<Option<u64>> {
    // Verify account exists
    let metadata = state.metadata.lock().await;
    metadata.get_account(account_id).await
        .map_err(|e| ServiceError::new(format!("Failed to check account: {}", e)))?
        .ok_or_else(|| ServiceError::new(format!("Account '{}' not found", account_id)))?;
    drop(metadata);

    // Get list of deltas
    let delta_files = state.storage.list_deltas(account_id).await
        .map_err(|e| ServiceError::new(format!("Failed to list deltas: {}", e)))?;

    if delta_files.is_empty() {
        return Ok(None);
    }

    // Parse nonces from filenames and find the maximum
    let mut max_nonce: Option<u64> = None;
    for filename in &delta_files {
        if let Some(nonce_str) = filename.strip_suffix(".json") {
            if let Ok(nonce) = nonce_str.parse::<u64>() {
                max_nonce = Some(max_nonce.map_or(nonce, |current| current.max(nonce)));
            }
        }
    }

    Ok(max_nonce)
}

/// Get account state
pub async fn get_state(state: &AppState, account_id: &str) -> ServiceResult<AccountState> {
    // Verify account exists
    let metadata = state.metadata.lock().await;
    metadata.get_account(account_id).await
        .map_err(|e| ServiceError::new(format!("Failed to check account: {}", e)))?
        .ok_or_else(|| ServiceError::new(format!("Account '{}' not found", account_id)))?;
    drop(metadata);

    // Fetch state from storage
    let account_state = state.storage.pull_state(account_id).await
        .map_err(|e| ServiceError::new(format!("Failed to fetch state: {}", e)))?;

    Ok(account_state)
}
