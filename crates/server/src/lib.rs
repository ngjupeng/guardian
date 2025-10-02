pub use private_state_manager_shared::{FromJson, ToJson};

use axum::{
  routing::{get, post},
  http::StatusCode,
  Json, Router,
};
use serde::{Deserialize, Serialize};

pub async fn run() {
  // initialize tracing
  // tracing_subscriber::fmt::init();
  let app = Router::new()
      .route("/", get(root))
      .route("/delta", post(push_delta))
      .route("/delta", get(get_delta))
      .route("/head", get(get_delta_head))
      .route("/configure", post(configure))
      .route("/state", get(get_state));

  // run our app with hyper, listening globally on port 3000
  let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
  println!("Listening on {}", listener.local_addr().unwrap());
  axum::serve(listener, app).await.unwrap();
}

async fn root() -> &'static str {
  "Hello, World!"
}

#[derive(Serialize, Deserialize)]
struct Delta {}

async fn push_delta(Json(payload): Json<Delta>) -> (StatusCode, Json<Delta>) {
  (StatusCode::OK, Json(payload))
}

async fn get_delta() -> (StatusCode, Json<Delta>) {
  let delta = Delta {};
  (StatusCode::OK, Json(delta))
}

async fn get_delta_head() -> (StatusCode, Json<Delta>) {
  let delta = Delta {};
  (StatusCode::OK, Json(delta))
}

async fn configure(Json(payload): Json<()>) -> (StatusCode, Json<()>) {
  (StatusCode::OK, Json(payload))
}

async fn get_state() -> (StatusCode, Json<()>) {
  (StatusCode::OK, Json(()))
}