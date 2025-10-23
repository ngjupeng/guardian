// Integration tests (enabled with `--features integration`)
#![cfg(feature = "integration")]

mod auth_http;
mod auth_grpc;
mod miden_rpc_integration;
