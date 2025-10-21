use crate::error::{PsmError, Result};
use crate::state::AppState;
use crate::storage::{AccountState, DeltaObject, DeltaStatus, StorageBackend};
use crate::auth::Auth;
use std::sync::Arc;
use miden_objects::{Word, utils::Serializable};
use private_state_manager_shared::FromJson;
use miden_objects::account::Account;

use super::{CandidateDelta, VerificationResult};

pub async fn process_candidates(
    state: &AppState,
    storage_backend: &Arc<dyn StorageBackend>,
    candidates: Vec<DeltaObject>,
    account_id: &str,
) -> Result<()> {
    for delta in candidates {
        let nonce = delta.nonce;
        let candidate = CandidateDelta::new(delta);
        if let Err(e) = process_candidate(state, storage_backend, candidate).await {
            eprintln!(
                "Failed to canonicalize delta {} for account {}: {}",
                nonce, account_id, e
            );
        }
    }
    Ok(())
}

async fn process_candidate(
    state: &AppState,
    storage_backend: &Arc<dyn StorageBackend>,
    candidate: CandidateDelta,
) -> Result<()> {
    let on_chain_commitment = fetch_on_chain_commitment(state, &candidate.delta.account_id).await?;
    let verification_result = candidate.verify(on_chain_commitment);

    match verification_result {
        VerificationResult::Matched(verified) => {
            canonicalize_verified_delta(state, storage_backend, &verified).await
        }
        VerificationResult::Mismatched {
            delta,
            expected_commitment,
            actual_commitment,
        } => {
            discard_mismatched_delta(
                storage_backend,
                delta,
                &expected_commitment,
                &actual_commitment,
            )
            .await
        }
    }
}

async fn fetch_on_chain_commitment(state: &AppState, account_id: &str) -> Result<String> {
    let mut client = state.network_client.lock().await;
    client
        .verify_on_chain_state(account_id)
        .await
        .map_err(PsmError::NetworkError)
}

fn extract_cosigner_pubkeys_from_storage(state_json: &serde_json::Value) -> Result<Option<Vec<String>>> {
    let account = Account::from_json(state_json)
        .map_err(|e| PsmError::InvalidDelta(format!("Failed to deserialize account: {}", e)))?;

    // Check if slot 1 contains a map by checking if the slot type is a map
    // If we can't get any item at index 0, slot 1 is not a map or is empty
    let key_zero = Word::from([0u32, 0, 0, 0]);
    let first_entry = account.storage().get_map_item(1, key_zero);

    // If we can't get the first entry or it's empty, slot 1 doesn't contain a valid map
    if first_entry.is_err() || first_entry.as_ref().unwrap() == &Word::default() {
        return Ok(None);
    }

    // Extract all public keys from the map
    let mut pubkeys = Vec::new();
    let mut index = 0u32;
    loop {
        let key = Word::from([index, 0, 0, 0]);
        match account.storage().get_map_item(1, key) {
            Ok(value) if value != Word::default() => {
                let pubkey_hex = format!("0x{}", hex::encode(value.to_bytes()));
                pubkeys.push(pubkey_hex);
                index += 1;
            }
            _ => break,
        }
    }

    if pubkeys.is_empty() {
        Ok(None)
    } else {
        Ok(Some(pubkeys))
    }
}

async fn canonicalize_verified_delta(
    state: &AppState,
    storage_backend: &Arc<dyn StorageBackend>,
    verified: &super::VerifiedDelta,
) -> Result<()> {
    let delta = verified.delta();

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
        state_json: new_state_json,
        commitment: new_commitment,
        created_at: current_state.created_at.clone(),
        updated_at: now.clone(),
    };

    storage_backend
        .submit_state(&updated_state)
        .await
        .map_err(|e| PsmError::StorageError(format!("Failed to update account state: {e}")))?;

    // Extract public keys from storage slot 1 and sync with metadata
    // Only proceed if slot 1 contains a valid map with public keys
    if let Some(storage_pubkeys) = extract_cosigner_pubkeys_from_storage(&updated_state.state_json)? {
        let current_metadata = state
            .metadata
            .get(&delta.account_id)
            .await
            .map_err(|e| PsmError::StorageError(format!("Failed to get metadata: {}", e)))?
            .ok_or_else(|| PsmError::AccountNotFound(delta.account_id.clone()))?;

        let metadata_pubkeys = match &current_metadata.auth {
            Auth::MidenFalconRpo { cosigner_pubkeys } => cosigner_pubkeys.clone(),
        };

        if storage_pubkeys != metadata_pubkeys {
            println!(
                "  Syncing cosigner public keys: metadata has {} keys, storage has {} keys",
                metadata_pubkeys.len(),
                storage_pubkeys.len()
            );

            let mut updated_metadata = current_metadata.clone();
            updated_metadata.auth = Auth::MidenFalconRpo {
                cosigner_pubkeys: storage_pubkeys,
            };
            updated_metadata.updated_at = now.clone();

            state
                .metadata
                .set(updated_metadata)
                .await
                .map_err(|e| PsmError::StorageError(format!("Failed to update metadata: {}", e)))?;

            println!("  ✓ Metadata cosigner public keys synced with storage");
        }
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
    expected_commitment: &str,
    actual_commitment: &str,
) -> Result<()> {
    println!(
        "✗ Discarding delta {} for account {} (commitment mismatch: expected {}, got {})",
        delta.nonce, delta.account_id, expected_commitment, actual_commitment
    );

    let now = chrono::Utc::now().to_rfc3339();

    let mut discarded_delta = delta.clone();
    discarded_delta.status = DeltaStatus::discarded(now);

    storage_backend
        .submit_delta(&discarded_delta)
        .await
        .map_err(|e| PsmError::StorageError(format!("Failed to update delta as discarded: {e}")))?;

    Err(PsmError::CommitmentMismatch {
        expected: expected_commitment.to_string(),
        actual: actual_commitment.to_string(),
    })
}
