use crate::auth::AuthType;
use crate::services;
use crate::state::AppState;
use crate::storage::{AccountState, DeltaObject};
use axum::{extract::Query, extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct ConfigureRequest {
    pub account_id: String,
    pub auth_type: AuthType,
    pub initial_state: serde_json::Value,
    pub storage_type: String,
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

// Response types
#[derive(Serialize)]
pub struct ConfigureResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub success: bool,
    pub error: String,
}

#[derive(Serialize)]
pub struct DeltaHeadResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_nonce: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

// ============================================================================
// HTTP Handlers
// ============================================================================

pub async fn configure(
    State(state): State<AppState>,
    Json(payload): Json<ConfigureRequest>,
) -> (StatusCode, Json<ConfigureResponse>) {
    match services::configure_account(
        &state,
        payload.account_id.clone(),
        payload.auth_type,
        payload.initial_state,
        payload.storage_type,
        payload.cosigner_pubkeys,
    )
    .await
    {
        Ok(_) => (
            StatusCode::OK,
            Json(ConfigureResponse {
                success: true,
                message: format!("Account '{}' configured successfully", payload.account_id),
            }),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ConfigureResponse {
                success: false,
                message: e.message,
            }),
        ),
    }
}

pub async fn push_delta(
    State(state): State<AppState>,
    Json(payload): Json<DeltaObject>,
) -> (StatusCode, Json<DeltaObject>) {
    match services::push_delta(&state, payload).await {
        Ok(delta) => (StatusCode::OK, Json(delta)),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(DeltaObject {
                account_id: e.message,
                ..Default::default()
            }),
        ),
    }
}

pub async fn get_delta(
    State(state): State<AppState>,
    Query(query): Query<DeltaQuery>,
) -> (StatusCode, Json<DeltaObject>) {
    match services::get_delta(&state, &query.account_id, query.nonce).await {
        Ok(delta) => (StatusCode::OK, Json(delta)),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(DeltaObject {
                account_id: e.message,
                ..Default::default()
            }),
        ),
    }
}

pub async fn get_delta_head(
    State(state): State<AppState>,
    Query(query): Query<StateQuery>,
) -> (StatusCode, Json<DeltaHeadResponse>) {
    match services::get_latest_nonce(&state, &query.account_id).await {
        Ok(latest_nonce) => (
            StatusCode::OK,
            Json(DeltaHeadResponse {
                success: true,
                latest_nonce,
                message: if latest_nonce.is_some() {
                    Some("Latest nonce retrieved successfully".to_string())
                } else {
                    Some("No deltas found for account".to_string())
                },
            }),
        ),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(DeltaHeadResponse {
                success: false,
                latest_nonce: None,
                message: Some(e.message),
            }),
        ),
    }
}

pub async fn get_state(
    State(state): State<AppState>,
    Query(query): Query<StateQuery>,
) -> (StatusCode, Json<AccountState>) {
    match services::get_state(&state, &query.account_id).await {
        Ok(account_state) => (StatusCode::OK, Json(account_state)),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(AccountState {
                account_id: e.message,
                ..Default::default()
            }),
        ),
    }
}
