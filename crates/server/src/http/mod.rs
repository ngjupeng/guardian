use crate::state::AppState;
use crate::storage::{AccountState, DeltaObject};
use axum::{extract::Query, extract::State, http::StatusCode, Json};
use serde::Deserialize;
// use crate::services;

#[derive(Deserialize)]
pub struct ConfigureRequest {
    pub account_id: String,
    pub initial_state: serde_json::Value,
    pub storage_type: String, // "local" or "S3"
    #[serde(default)]
    pub cosigner_pubkeys: Vec<String>,
}

#[derive(Deserialize)]
pub struct DeltaQuery {
    pub account_id: String,
    pub nonce: u64,
}

#[derive(Deserialize)]
pub struct StateQuery {
    pub account_id: String,
}

// ============================================================================
// HTTP Handlers
// ============================================================================

pub async fn configure(
    State(_state): State<AppState>,
    Json(_payload): Json<ConfigureRequest>,
) -> StatusCode {
    // TODO: Implement
    StatusCode::OK
}

pub async fn push_delta(
    State(_state): State<AppState>,
    Json(_payload): Json<DeltaObject>,
) -> (StatusCode, Json<DeltaObject>) {
    // TODO: Implement
    (StatusCode::OK, Json(DeltaObject::default()))
}

pub async fn get_delta(
    State(_state): State<AppState>,
    Query(_query): Query<DeltaQuery>,
) -> (StatusCode, Json<DeltaObject>) {
    // TODO: Implement
    (StatusCode::OK, Json(DeltaObject::default()))
}

pub async fn get_delta_head(
    State(_state): State<AppState>,
    Query(_query): Query<StateQuery>,
) -> (StatusCode, Json<DeltaObject>) {
    // TODO: Implement
    (StatusCode::OK, Json(DeltaObject::default()))
}

pub async fn get_state(
    State(_state): State<AppState>,
    Query(_query): Query<StateQuery>,
) -> (StatusCode, Json<AccountState>) {
    // TODO: Implement
    (StatusCode::OK, Json(AccountState::default()))
}
