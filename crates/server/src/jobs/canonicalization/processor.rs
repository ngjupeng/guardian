use crate::auth;
use crate::error::{PsmError, Result};
use crate::state::AppState;
use crate::storage::{AccountState, DeltaObject, DeltaStatus, StorageBackend};
use std::sync::Arc;

pub async fn process_candidates(
    state: &AppState,
    storage_backend: &Arc<dyn StorageBackend>,
    candidates: Vec<DeltaObject>,
    account_id: &str,
) -> Result<()> {
    for delta in candidates {
        let nonce = delta.nonce;
        if let Err(e) = process_candidate(state, storage_backend, delta).await {
            eprintln!("Failed to canonicalize delta {nonce} for account {account_id}: {e}");
        }
    }
    Ok(())
}

async fn process_candidate(
    state: &AppState,
    storage_backend: &Arc<dyn StorageBackend>,
    delta: DeltaObject,
) -> Result<()> {
    let is_canonical = {
        let mut client = state.network_client.lock().await;
        client
            .is_canonical(&delta)
            .await
            .map_err(PsmError::NetworkError)?
    };

    if is_canonical {
        canonicalize_verified_delta(state, storage_backend, delta).await
    } else {
        discard_mismatched_delta(storage_backend, delta).await
    }
}

async fn canonicalize_verified_delta(
    state: &AppState,
    storage_backend: &Arc<dyn StorageBackend>,
    delta: DeltaObject,
) -> Result<()> {
    println!(
        "✓ Canonicalizing delta {} for account {} (commitment matches on-chain)",
        delta.nonce, delta.account_id
    );

    let current_state = storage_backend
        .pull_state(&delta.account_id)
        .await
        .map_err(|e| PsmError::StorageError(format!("Failed to get current state: {e}")))?;

    let (new_state_json, new_commitment) = {
        let client = state.network_client.lock().await;
        client
            .apply_delta(&current_state.state_json, &delta.delta_payload)
            .map_err(PsmError::InvalidDelta)?
    };

    let now = chrono::Utc::now().to_rfc3339();

    let updated_state = AccountState {
        account_id: delta.account_id.clone(),
        state_json: new_state_json.clone(),
        commitment: new_commitment,
        created_at: current_state.created_at.clone(),
        updated_at: now.clone(),
    };

    storage_backend
        .submit_state(&updated_state)
        .await
        .map_err(|e| PsmError::StorageError(format!("Failed to update account state: {e}")))?;

    let new_auth_creds = {
        let mut client = state.network_client.lock().await;
        client
            .should_update_auth(&new_state_json)
            .await
            .map_err(|e| PsmError::StorageError(format!("Failed to check auth update: {e}")))?
    };

    if let Some(new_creds) = new_auth_creds {
        println!(
            "  Syncing cosigner public keys from on-chain storage"
        );

        auth::update_credentials(&*state.metadata, &delta.account_id, new_creds, &now)
            .await?;

        println!("  ✓ Metadata cosigner public keys synced with storage");
    }

    let mut canonical_delta = delta.clone();
    canonical_delta.status = DeltaStatus::canonical(now);

    storage_backend
        .submit_delta(&canonical_delta)
        .await
        .map_err(|e| PsmError::StorageError(format!("Failed to update delta as canonical: {e}")))?;

    Ok(())
}

async fn discard_mismatched_delta(
    storage_backend: &Arc<dyn StorageBackend>,
    delta: DeltaObject,
) -> Result<()> {
    println!(
        "✗ Discarding delta {} for account {} (commitment mismatch with on-chain state)",
        delta.nonce, delta.account_id
    );

    let now = chrono::Utc::now().to_rfc3339();

    let mut discarded_delta = delta.clone();
    discarded_delta.status = DeltaStatus::discarded(now);

    storage_backend
        .submit_delta(&discarded_delta)
        .await
        .map_err(|e| PsmError::StorageError(format!("Failed to update delta as discarded: {e}")))?;

    Ok(())
}
