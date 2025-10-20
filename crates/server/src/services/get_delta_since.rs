use crate::auth::Credentials;
use crate::state::AppState;
use crate::storage::DeltaObject;

use super::common::{ServiceError, ServiceResult, verify_request_auth};

#[derive(Debug, Clone)]
pub struct GetDeltaSinceParams {
    pub account_id: String,
    pub from_nonce: u64,
    pub credentials: Credentials,
}

#[derive(Debug, Clone)]
pub struct GetDeltaSinceResult {
    pub merged_delta: DeltaObject,
}

pub async fn get_delta_since(
    state: &AppState,
    params: GetDeltaSinceParams,
) -> ServiceResult<GetDeltaSinceResult> {
    let account_metadata = state
        .metadata
        .get(&params.account_id)
        .await
        .map_err(|e| ServiceError::new(format!("Failed to check account: {e}")))?
        .ok_or_else(|| ServiceError::new(format!("Account '{}' not found", params.account_id)))?;

    verify_request_auth(
        &account_metadata.auth,
        &params.account_id,
        &params.credentials,
    )?;

    let storage_backend = state
        .storage
        .get(&account_metadata.storage_type)
        .map_err(ServiceError::new)?;

    let all_deltas = storage_backend
        .pull_deltas_after(&params.account_id, params.from_nonce)
        .await
        .map_err(|e| ServiceError::new(format!("Failed to fetch deltas: {e}")))?;

    // Filter out discarded deltas
    let deltas: Vec<DeltaObject> = all_deltas
        .into_iter()
        .filter(|delta| delta.discarded_at.is_none())
        .collect();

    if deltas.is_empty() {
        return Err(ServiceError::new(format!(
            "No valid deltas found after nonce {}",
            params.from_nonce
        )));
    }

    let delta_payloads: Vec<serde_json::Value> =
        deltas.iter().map(|d| d.delta_payload.clone()).collect();

    let merged_payload = {
        let client = state.network_client.lock().await;
        client
            .merge_deltas(delta_payloads)
            .map_err(|e| ServiceError::new(format!("Failed to merge deltas: {e}")))?
    };

    let last_delta = deltas.last().unwrap();

    let merged_delta = DeltaObject {
        account_id: params.account_id,
        nonce: last_delta.nonce,
        prev_commitment: deltas.first().unwrap().prev_commitment.clone(),
        new_commitment: last_delta.new_commitment.clone(),
        delta_payload: merged_payload,
        ack_sig: last_delta.ack_sig.clone(),
        candidate_at: last_delta.candidate_at.clone(),
        canonical_at: last_delta.canonical_at.clone(),
        discarded_at: last_delta.discarded_at.clone(),
    };

    Ok(GetDeltaSinceResult { merged_delta })
}
