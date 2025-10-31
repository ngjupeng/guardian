mod falcon;
mod miden_helpers;
mod multisig;

use miden_helpers::{add_account_and_sync, commitment_from_hex, create_miden_client};
use miden_client::account::Account;
use miden_client::crypto::rpo_falcon512::PublicKey;
use miden_client::keystore::FilesystemKeyStore;
use miden_client::rpc::Endpoint;
use miden_client::{Deserializable, Felt, Serializable, Word};


use miden_objects::account::Signature as AccountSignature;
use miden_objects::crypto::dsa::rpo_falcon512::Signature as RawFalconSignature;

use private_state_manager_client::auth_config::AuthType;
use private_state_manager_client::{
    verify_commitment_signature, Auth, AuthConfig, ClientResult, FalconRpoSigner,
    MidenFalconRpoAuth, PsmClient,
};
use private_state_manager_shared::hex::FromHex;
use private_state_manager_shared::ToJson;

use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use tempfile::TempDir;

#[tokio::main]
async fn main() -> ClientResult<()> {
    println!("=== PSM Multi-Client E2E Flow ===\n");

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let rng = ChaCha20Rng::from_seed([42u8; 32]);
    let keystore = FilesystemKeyStore::with_rng(temp_dir.path().to_path_buf(), rng)
        .expect("Failed to create keystore");

    // =========================================================================
    // Setup: Generate keys for both clients
    // =========================================================================
    println!("Setup: Generating keys...");

    let (client1_full_pubkey_hex, client1_commitment_hex, client1_secret_key) =
        falcon::generate_falcon_keypair(&keystore);
    let (client2_full_pubkey_hex, client2_commitment_hex, client2_secret_key) =
        falcon::generate_falcon_keypair(&keystore);

    println!("  ✓ Client 1 commitment: {}...", &client1_commitment_hex);
    println!("  ✓ Client 2 commitment: {}...", &client2_commitment_hex);
    println!();

    // =========================================================================
    // Step 1: Connect to PSM and get server's public key
    // =========================================================================
    println!("Step 1: Connect to PSM and get server's public key...");

    let client1_signer = FalconRpoSigner::new(client1_secret_key.clone());
    let client1_auth = Auth::FalconRpoSigner(client1_signer);

    let psm_endpoint = "http://localhost:50051".to_string();
    let mut client1 = match PsmClient::connect(psm_endpoint.clone()).await {
        Ok(client) => client.with_auth(client1_auth),
        Err(e) => {
            println!("  ✗ Failed to connect: {}", e);
            println!("  Hint: Start PSM server with: cargo run --package private-state-manager-server --bin server");
            return Ok(());
        }
    };

    let server_ack_pubkey = match client1.get_pubkey().await {
        Ok(pubkey) => {
            println!("  ✓ Connected to PSM server");
            pubkey
        }
        Err(e) => {
            println!("  ✗ Failed to get server pubkey: {}", e);
            return Ok(());
        }
    };

    // Compute the commitment from the server's full public key
    // The server returns the full key for signature verification, but we need
    // to store only the commitment in the account
    let server_pubkey_bytes =
        hex::decode(&server_ack_pubkey[2..]).expect("Failed to decode server public key");
    let server_pubkey = PublicKey::read_from_bytes(&server_pubkey_bytes)
        .expect("Failed to deserialize server public key");
    let server_commitment = server_pubkey.to_commitment();
    let server_commitment_hex = format!("0x{}", hex::encode(server_commitment.to_bytes()));

    println!("  ✓ Server commitment: {}...", &server_commitment_hex);
    println!();

    // =========================================================================
    // Step 2: Create multisig PSM account with server's pubkey commitment
    // =========================================================================
    println!("Step 2: Creating multisig PSM account with PSM auth...");

    let init_seed = [0xff; 32];
    let account = multisig::create_multisig_psm_account(
        &client1_commitment_hex,
        &client2_commitment_hex,
        &server_commitment_hex,
        init_seed,
    );

    let account_id = account.id();
    println!("  ✓ Account ID: {}", account_id);
    println!(
        "  ✓ Commitment: 0x{}",
        hex::encode(account.commitment().as_bytes())
    );
    println!("  ✓ Multisig: 2-of-2 (client1, client2)");
    println!("  ✓ PSM auth enabled with server's pubkey");
    println!();

    // =========================================================================
    // Step 3: Client 1 - Configure account in PSM
    // =========================================================================
    println!("Step 3: Client 1 - Configure account in PSM...");

    // Configure with both cosigners (use full public keys for auth, not commitments)
    // The server needs full keys to verify signatures
    let auth_config = AuthConfig {
        auth_type: Some(AuthType::MidenFalconRpo(MidenFalconRpoAuth {
            cosigner_pubkeys: vec![
                client1_full_pubkey_hex.clone(),
                client2_full_pubkey_hex.clone(),
            ],
        })),
    };

    // Create state with serialized account
    let account_bytes = account.to_bytes();
    let account_base64 =
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &account_bytes);

    let initial_state = serde_json::json!({
        "data": account_base64,
        "account_id": account_id.to_string(),
    });

    match client1
        .configure(&account_id, auth_config, initial_state, "Filesystem")
        .await
    {
        Ok(response) => {
            println!("  ✓ {}", response.message);
        }
        Err(e) => {
            println!("  ✗ Configuration failed: {}", e);
            return Ok(());
        }
    };
    println!();

    // =========================================================================
    // Step 4: Client 2 - Pull state from PSM
    // =========================================================================
    println!("Step 4: Client 2 - Pull state from PSM...");

    // Client 2 connects with their key
    let client2_signer = FalconRpoSigner::new(client2_secret_key.clone());
    let client2_auth = Auth::FalconRpoSigner(client2_signer);

    let mut client2 = PsmClient::connect(psm_endpoint.clone())
        .await
        .expect("Failed to connect")
        .with_auth(client2_auth);

    let retrieved_account = match client2.get_state(&account_id).await {
        Ok(response) => {
            println!("  ✓ {}", response.message);
            if let Some(state) = response.state {
                println!("    Commitment: {}", state.commitment);
                println!("    Updated at: {}", state.updated_at);

                let state_value: serde_json::Value =
                    serde_json::from_str(&state.state_json).expect("Failed to parse state_json");

                // Deserialize account
                if let Some(data_str) = state_value["data"].as_str() {
                    let bytes = base64::Engine::decode(
                        &base64::engine::general_purpose::STANDARD,
                        data_str,
                    )
                    .expect("Failed to decode account data");
                    match Account::read_from_bytes(&bytes) {
                        Ok(account) => {
                            println!("    ✓ Deserialized account");
                            Some(account)
                        }
                        Err(e) => {
                            println!("    ✗ Failed to deserialize: {}", e);
                            None
                        }
                    }
                } else {
                    None
                }
            } else {
                None
            }
        }
        Err(e) => {
            println!("  ✗ Failed to get state: {}", e);
            None
        }
    };
    println!();

    // =========================================================================
    // Step 5: Client 2 - Create TransactionSummary
    // =========================================================================
    if let Some(account) = retrieved_account {
        println!("Step 5: Client 2 - Create TransactionSummary for 3-of-3 + new cosigner...");
        println!("  ✓ Account retrieved from PSM");
        println!("    Account ID: {}", account.id());
        println!("    Current nonce: {}", account.nonce());



        // Generate a new cosigner keypair
        println!("  Generating new cosigner keypair...");
        let (_new_cosigner_full_pubkey_hex, new_cosigner_commitment_hex, _new_cosigner_secret_key) =
            falcon::generate_falcon_keypair(&keystore);
        println!(
            "  ✓ New cosigner commitment: {}...",
            &new_cosigner_commitment_hex
        );

        println!("  Preparing multisig update payload (3-of-3 with new cosigner)...");

        let signer_commitments = match [&client1_commitment_hex, &client2_commitment_hex, &new_cosigner_commitment_hex]
            .into_iter()
            .map(|hex_commitment| commitment_from_hex(hex_commitment))
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(commitments) => commitments,
            Err(err) => {
                println!("  ✗ Failed to parse signer commitments: {}", err);
                return Ok(());
            }
        };

        let salt = Word::from([
            Felt::new(42),
            Felt::new(0),
            Felt::new(0),
            Felt::new(0),
        ]);

        let (simulation_request, config_hash) = match multisig::build_update_signers_transaction_request(
            3,
            &signer_commitments,
            salt,
            std::iter::empty::<(Word, Vec<Felt>)>(),
        ) {
            Ok(result) => result,
            Err(err) => {
                println!("  ✗ Failed to build simulation request: {}", err);
                return Ok(());
            }
        };

        println!(
            "  ✓ Auth SALT: 0x{}",
            hex::encode(salt.to_bytes())
        );
        println!(
            "  ✓ Multisig config hash: 0x{}",
            hex::encode(config_hash.to_bytes())
        );

        println!("  Setting up local Miden client for simulation...");
        let local_endpoint = Endpoint::new("http".to_string(), "127.0.0.1".to_string(), Some(57291));
        let mut sim_client = match create_miden_client(temp_dir.path(), &local_endpoint).await {
            Ok(client) => client,
            Err(err) => {
                println!("  ✗ Failed to create Miden client: {}", err);
                println!(
                    "    Hint: ensure a local node is running at {}",
                    local_endpoint
                );
                return Ok(());
            }
        };

        if let Err(err) = add_account_and_sync(&mut sim_client, &account).await {
            println!("  ✗ Failed to add or sync account in Miden client: {}", err);
            return Ok(());
        }

        println!("  Executing dry-run transaction (expect Unauthorized)...");
        let tx_summary = match multisig::execute_transaction_for_summary(
            &mut sim_client,
            account.id(),
            simulation_request,
        )
        .await
        {
            Ok(summary) => summary,
            Err(err) => {
                println!("  ✗ Simulation failed: {}", err);
                return Ok(());
            }
        };

        println!(
            "  ✓ TX summary commitment: 0x{}",
            hex::encode(tx_summary.to_commitment().to_bytes())
        );
        println!();

        // =========================================================================
        // Step 6: Client 2 - Push TransactionSummary to PSM; server signs TX summary commitment
        // =========================================================================
        println!(
            "Step 6: Push TransactionSummary; expect ack signature over TX_SUMMARY_COMMITMENT..."
        );

        // Serialize TransactionSummary to JSON using the expected format
        let tx_summary_json = tx_summary.to_json();
        let prev_commitment = format!("0x{}", hex::encode(account.commitment().as_bytes()));

        let (_new_commitment, ack_sig) = match client2
            .push_delta(
                &account_id,
                account.nonce().as_int(),
                prev_commitment,
                tx_summary_json,
            )
            .await
        {
            Ok(response) => {
                println!("  ✓ {}", response.message);
                // ack_sig can be at top level or in delta
                let ack_sig = response
                    .ack_sig
                    .or_else(|| response.delta.as_ref().map(|d| d.ack_sig.clone()))
                    .unwrap_or_default();

                if let Some(delta) = response.delta {
                    println!("    New commitment: {}", delta.new_commitment);
                    if !ack_sig.is_empty() {
                        println!(
                            "    PSM ack signature: {}...",
                            &ack_sig[0..20.min(ack_sig.len())]
                        );
                        (delta.new_commitment, ack_sig)
                    } else {
                        println!("  ✗ Missing ack signature in response");
                        return Ok(());
                    }
                } else {
                    println!("  ✗ No delta in response");
                    return Ok(());
                }
            }
            Err(e) => {
                println!("  ✗ Failed to push TransactionSummary: {}", e);
                return Ok(());
            }
        };
        println!();

        // =========================================================================
        // Step 7: Verify PSM ack signature over the TX summary commitment; update local account
        // =========================================================================
        println!("Step 7: Verify PSM ack over TX summary commitment...");

        // Compute TX summary commitment hex for verification
        let tx_summary_commitment_hex =
            format!("0x{}", hex::encode(tx_summary.to_commitment().to_bytes()));

        // Ensure the ack_sig has 0x prefix for verify_commitment_signature
        let ack_sig_with_prefix = if ack_sig.starts_with("0x") {
            ack_sig.clone()
        } else {
            format!("0x{}", ack_sig)
        };

        println!(
            "  Debug: TX commitment for verification: {}",
            &tx_summary_commitment_hex
        );
        println!(
            "  Debug: Server pubkey: {}...",
            &server_ack_pubkey[..40.min(server_ack_pubkey.len())]
        );
        println!(
            "  Debug: ACK signature: {}...",
            &ack_sig_with_prefix[..40.min(ack_sig_with_prefix.len())]
        );

        match verify_commitment_signature(
            &tx_summary_commitment_hex,
            &server_ack_pubkey,
            &ack_sig_with_prefix,
        ) {
            Ok(true) => {
                println!("  ✓ PSM signature VALID over TX summary commitment");

                let tx_message = tx_summary.to_commitment();

                let ack_signature = match RawFalconSignature::from_hex(&ack_sig_with_prefix) {
                    Ok(sig) => AccountSignature::from(sig),
                    Err(err) => {
                        println!("  ✗ Failed to parse PSM signature: {}", err);
                        return Ok(());
                    }
                };

                let cosigner1_signature = AccountSignature::from(client1_secret_key.sign(tx_message));
                let cosigner2_signature = AccountSignature::from(client2_secret_key.sign(tx_message));

                let mut signature_advice = Vec::new();
                signature_advice.push(multisig::build_signature_advice_entry(
                    server_commitment,
                    tx_message,
                    &ack_signature,
                ));
                signature_advice.push(multisig::build_signature_advice_entry(
                    signer_commitments[0],
                    tx_message,
                    &cosigner1_signature,
                ));
                signature_advice.push(multisig::build_signature_advice_entry(
                    signer_commitments[1],
                    tx_message,
                    &cosigner2_signature,
                ));

                println!("Step 8: Building final transaction request with signatures...");
                let (final_request, _) = match multisig::build_update_signers_transaction_request(
                    3,
                    &signer_commitments,
                    salt,
                    signature_advice,
                ) {
                    Ok(result) => result,
                    Err(err) => {
                        println!("  ✗ Failed to build final transaction request: {}", err);
                        return Ok(());
                    }
                };

                println!("  Executing transaction with signatures...");
                let tx_result = match sim_client
                    .new_transaction(account.id(), final_request)
                    .await
                {
                    Ok(result) => {
                        println!("  ✓ Transaction executed successfully");
                        result
                    }
                    Err(e) => {
                        println!("  ✗ Transaction execution failed: {:?}", e);
                        return Ok(());
                    }
                };

                let executed_tx = tx_result.executed_transaction();
                let final_commitment = executed_tx.final_account().commitment();
                let nonce_delta = executed_tx.account_delta().nonce_delta();

                println!("  Submitting transaction to the node...");
                if let Err(e) = sim_client.submit_transaction(tx_result).await {
                    println!("  ✗ Failed to submit transaction: {}", e);
                    return Ok(());
                }

                println!("  ✓ Transaction submitted");
                println!(
                    "    Final account commitment: 0x{}",
                    hex::encode(final_commitment.as_bytes())
                );
                println!("    Nonce delta: {}", nonce_delta.as_int());
            }
            Ok(false) => {
                println!("  ✗ PSM signature INVALID");
            }
            Err(e) => {
                println!("  ✗ Signature verification error: {}", e);
            }
        }
    } else {
        println!("  ✗ Failed to retrieve account from PSM");
    }

    println!("\n=== Multi-client E2E flow completed! ===");
    Ok(())
}
