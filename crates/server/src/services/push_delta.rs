use crate::auth::Credentials;
use crate::canonicalization::CanonicalizationMode;
use crate::error::{PsmError, Result};
use crate::services::resolve_account;
use crate::state::AppState;
use crate::storage::{AccountState, DeltaObject, DeltaStatus, StorageBackend};
use miden_objects::{Felt, Word, crypto::hash::rpo::Rpo256, utils::Serializable};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct PushDeltaParams {
    pub delta: DeltaObject,
    pub credentials: Credentials,
}

#[derive(Debug, Clone)]
pub struct PushDeltaResult {
    pub delta: DeltaObject,
}

pub async fn push_delta(state: &AppState, params: PushDeltaParams) -> Result<PushDeltaResult> {
    let resolved = resolve_account(state, &params.delta.account_id, &params.credentials).await?;

    check_no_pending_candidates(&resolved.backend, &params.delta.account_id).await?;

    let current_state = resolved
        .backend
        .pull_state(&params.delta.account_id)
        .await
        .map_err(|e| PsmError::StorageError(format!("Failed to fetch account state: {e}")))?;

    verify_prev_commitment(&params.delta, &current_state)?;

    let new_commitment =
        calculate_new_commitment(state, &current_state, &params.delta.delta_payload).await?;

    let now = state.clock.now_rfc3339();

    match &state.canonicalization_mode {
        CanonicalizationMode::Enabled(_) => {
            save_as_candidate(&resolved.backend, &params.delta, &new_commitment, &now).await?;
        }
        CanonicalizationMode::Optimistic => {
            save_as_canonical(
                state,
                &resolved.backend,
                &params.delta,
                &current_state,
                &new_commitment,
                &now,
            )
            .await?;
        }
    }

    Ok(PushDeltaResult {
        delta: params.delta,
    })
}

async fn check_no_pending_candidates(
    storage_backend: &Arc<dyn StorageBackend>,
    account_id: &str,
) -> Result<()> {
    let all_deltas = storage_backend
        .pull_deltas_after(account_id, 0)
        .await
        .map_err(|e| PsmError::StorageError(format!("Failed to check deltas: {e}")))?;

    tracing::debug!(
        account_id = %account_id,
        delta_count = all_deltas.len(),
        "Checking existing deltas"
    );

    let has_pending_candidate = all_deltas.iter().any(|d| d.status.is_candidate());

    if has_pending_candidate {
        return Err(PsmError::ConflictPendingDelta);
    }

    Ok(())
}

fn verify_prev_commitment(delta: &DeltaObject, current_state: &AccountState) -> Result<()> {
    if delta.prev_commitment != current_state.commitment {
        return Err(PsmError::CommitmentMismatch {
            expected: current_state.commitment.clone(),
            actual: delta.prev_commitment.clone(),
        });
    }
    Ok(())
}

async fn calculate_new_commitment(
    state: &AppState,
    current_state: &AccountState,
    delta_payload: &serde_json::Value,
) -> Result<String> {
    let client = state.network_client.lock().await;

    client
        .verify_delta(
            &current_state.commitment,
            &current_state.state_json,
            delta_payload,
        )
        .map_err(PsmError::InvalidDelta)?;

    let (_, commitment) = client
        .apply_delta(&current_state.state_json, delta_payload)
        .map_err(PsmError::InvalidDelta)?;

    Ok(commitment)
}

async fn save_as_candidate(
    storage_backend: &Arc<dyn StorageBackend>,
    delta: &DeltaObject,
    new_commitment: &str,
    timestamp: &str,
) -> Result<()> {
    let mut candidate_delta = delta.clone();
    candidate_delta.new_commitment = new_commitment.to_string();
    candidate_delta.status = DeltaStatus::candidate(timestamp.to_string());

    storage_backend
        .submit_delta(&candidate_delta)
        .await
        .map_err(|e| PsmError::StorageError(format!("Failed to submit delta: {e}")))
}

async fn save_as_canonical(
    state: &AppState,
    storage_backend: &Arc<dyn StorageBackend>,
    delta: &DeltaObject,
    current_state: &AccountState,
    new_commitment: &str,
    timestamp: &str,
) -> Result<()> {
    let mut canonical_delta = delta.clone();
    canonical_delta.new_commitment = new_commitment.to_string();
    canonical_delta.status = DeltaStatus::canonical(timestamp.to_string());

    let commitment_digest = commitment_to_digest(new_commitment)?;
    let signature = state
        .signing
        .sign_with_server_key(commitment_digest)
        .map_err(|e| PsmError::SigningError(format!("Failed to sign commitment: {e}")))?;

    canonical_delta.ack_sig = Some(hex::encode(signature.to_bytes()));

    let (new_state_json, _) = {
        let client = state.network_client.lock().await;
        client
            .apply_delta(&current_state.state_json, &canonical_delta.delta_payload)
            .map_err(PsmError::InvalidDelta)?
    };

    let new_state = AccountState {
        account_id: canonical_delta.account_id.clone(),
        commitment: new_commitment.to_string(),
        state_json: new_state_json,
        created_at: current_state.created_at.clone(),
        updated_at: timestamp.to_string(),
    };

    storage_backend
        .submit_state(&new_state)
        .await
        .map_err(|e| PsmError::StorageError(format!("Failed to update state: {e}")))?;

    storage_backend
        .submit_delta(&canonical_delta)
        .await
        .map_err(|e| PsmError::StorageError(format!("Failed to submit delta: {e}")))?;

    Ok(())
}

fn commitment_to_digest(commitment_hex: &str) -> Result<Word> {
    let commitment_hex = commitment_hex.strip_prefix("0x").unwrap_or(commitment_hex);

    let bytes = hex::decode(commitment_hex)
        .map_err(|e| PsmError::InvalidCommitment(format!("Invalid hex: {e}")))?;

    if bytes.len() != 32 {
        return Err(PsmError::InvalidCommitment(format!(
            "Commitment must be 32 bytes, got {}",
            bytes.len()
        )));
    }

    let mut felts = Vec::new();
    for chunk in bytes.chunks(8) {
        let mut arr = [0u8; 8];
        arr[..chunk.len()].copy_from_slice(chunk);
        let value = u64::from_le_bytes(arr);
        felts.push(
            Felt::try_from(value)
                .map_err(|e| PsmError::InvalidCommitment(format!("Invalid field element: {e}")))?,
        );
    }

    let message_elements = vec![felts[0], felts[1], felts[2], felts[3]];

    let digest = Rpo256::hash_elements(&message_elements);
    Ok(digest)
}
