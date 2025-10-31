use std::path::Path;
use std::sync::Arc;

use miden_client::account::Account;
use miden_client::crypto::RpoRandomCoin;
use miden_client::rpc::{Endpoint, GrpcClient, NodeRpcClient};
use miden_client::transaction::TransactionAuthenticator;
use miden_client::{Client, ClientError, Deserializable, ExecutionOptions, Word};
use miden_objects::{MAX_TX_EXECUTION_CYCLES, MIN_TX_EXECUTION_CYCLES};
use miden_client_sqlite_store::SqliteStore;

/// Instantiate a `Client<()>` configured for the provided endpoint.
pub async fn create_miden_client(
    data_dir: &Path,
    endpoint: &Endpoint,
) -> Result<Client<()>, String> {
    let store_path = data_dir.join("miden-client.sqlite");
    let store = SqliteStore::new(store_path)
        .await
        .map_err(|err| format!("Failed to open SQLite store: {err}"))?;
    let store = Arc::new(store);

    let rng = Box::new(RpoRandomCoin::new(Word::default()));
    let exec_options = ExecutionOptions::new(
        Some(MAX_TX_EXECUTION_CYCLES),
        MIN_TX_EXECUTION_CYCLES,
        true,
        true,
    )
    .map_err(|err| format!("Failed to build execution options: {err}"))?;
    let rpc_client: Arc<dyn NodeRpcClient> = Arc::new(GrpcClient::new(endpoint, 10_000));

    Client::new(
        rpc_client,
        rng,
        store,
        None,
        exec_options,
        Some(20),
        Some(256),
        None,
    )
    .await
    .map_err(|err| format!("Failed to create Miden client: {err}"))
}

/// Adds the provided account to the client and synchronizes it with the network.
pub async fn add_account_and_sync<AUTH>(
    client: &mut Client<AUTH>,
    account: &Account,
) -> Result<(), ClientError>
where
    AUTH: TransactionAuthenticator + Sync + 'static,
{
    client.add_account(account, false).await?;
    client.sync_state().await?;
    Ok(())
}

/// Converts a hex-encoded public key commitment (with or without 0x prefix) into a `Word`.
pub fn commitment_from_hex(hex_commitment: &str) -> Result<Word, String> {
    let trimmed = hex_commitment.strip_prefix("0x").unwrap_or(hex_commitment);
    let bytes = hex::decode(trimmed)
        .map_err(|err| format!("Failed to decode commitment hex '{hex_commitment}': {err}"))?;

    Word::read_from_bytes(&bytes)
        .map_err(|err| format!("Failed to deserialize commitment word '{hex_commitment}': {err}"))
}

