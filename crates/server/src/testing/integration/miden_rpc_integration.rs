use crate::network::miden::MidenNetworkClient;
use crate::network::NetworkType;

/// Integration test for verifying we can connect to Miden testnet
/// To run: cargo test --package private-state-manager-server --test miden_rpc_integration_test
#[tokio::test]
async fn test_fetch_account_commitment_from_testnet() {
    let _client = MidenNetworkClient::from_network(NetworkType::MidenTestnet)
        .await
        .expect("Failed to create Miden network client");

    // Also perform a direct RPC call to assert connectivity
    let endpoint = NetworkType::MidenTestnet.rpc_endpoint();
    let mut rpc_client = miden_rpc_client::MidenRpcClient::connect(endpoint)
        .await
        .expect("Failed to connect RPC client");
    rpc_client
        .get_status()
        .await
        .expect("Status RPC call failed");
}
