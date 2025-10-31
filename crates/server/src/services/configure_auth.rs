use crate::error::{PsmError, Result};
use crate::metadata::auth::{Auth, Credentials};
use crate::state::AppState;

#[derive(Debug, Clone)]
pub struct ConfigureAuthParams {
    pub account_id: String,
    pub auth: Auth,
    pub credential: Credentials,
}

#[derive(Debug, Clone)]
pub struct ConfigureAuthResult {
    pub account_id: String,
    pub success: bool,
}

/// Configure authentication for an existing account
pub async fn configure_auth(
    state: &AppState,
    params: ConfigureAuthParams,
) -> Result<ConfigureAuthResult> {
    tracing::info!("Configuring auth for account: {}", params.account_id);

    let metadata = state
        .metadata
        .get(&params.account_id)
        .await
        .map_err(|e| PsmError::StorageError(format!("Failed to get account metadata: {e}")))?
        .ok_or_else(|| {
            PsmError::AccountNotFound(format!("Account {} not found", params.account_id))
        })?;

    metadata
        .auth
        .verify(&params.account_id, &params.credential)
        .map_err(|e| {
            PsmError::AuthenticationFailed(format!(
                "Authentication failed with existing credentials: {e}"
            ))
        })?;

    let storage_backend = state
        .storage
        .get(&metadata.storage_type)
        .map_err(PsmError::ConfigurationError)?;

    let current_state = storage_backend
        .pull_state(&params.account_id)
        .await
        .map_err(|e| PsmError::StorageError(format!("Failed to pull current state: {e}")))?;

    // Validate credential and new auth configuration against current state
    {
        let client = state.network_client.lock().await;
        client
            .validate_credential(&current_state.state_json, &params.credential)
            .map_err(|e| PsmError::NetworkError(format!("Failed to validate credential: {e}")))?;

        client
            .validate_auth_config(&current_state.state_json, &params.auth)
            .map_err(|e| {
                PsmError::AuthenticationFailed(format!(
                    "New auth configuration doesn't match account state: {e}"
                ))
            })?;
    }

    let now = state.clock.now_rfc3339();

    state
        .metadata
        .update_auth(&params.account_id, params.auth, &now)
        .await
        .map_err(|e| PsmError::StorageError(format!("Failed to update auth metadata: {e}")))?;

    tracing::info!(
        "Auth configuration updated successfully for account: {}",
        params.account_id
    );

    Ok(ConfigureAuthResult {
        account_id: params.account_id,
        success: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ack::{Acknowledger, MidenFalconRpoSigner};
    use crate::metadata::AccountMetadata;
    use crate::state_object::StateObject;
    use crate::storage::{StorageBackend, StorageRegistry, StorageType};
    use crate::testing::helpers::generate_falcon_signature;
    use crate::testing::mocks::{MockMetadataStore, MockNetworkClient, MockStorageBackend};
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    fn create_test_app_state(
        network_client: MockNetworkClient,
        storage_backend: MockStorageBackend,
        metadata_store: MockMetadataStore,
    ) -> AppState {
        let mut backends = HashMap::new();
        backends.insert(
            StorageType::Filesystem,
            Arc::new(storage_backend) as Arc<dyn StorageBackend>,
        );

        let keystore_dir =
            std::env::temp_dir().join(format!("test_keystore_{}", uuid::Uuid::new_v4()));

        let signer = MidenFalconRpoSigner::new(keystore_dir).expect("Failed to create signer");
        let ack = Acknowledger::FilesystemMidenFalconRpo(signer);

        AppState {
            storage: StorageRegistry::new(backends),
            metadata: Arc::new(metadata_store),
            network_client: Arc::new(Mutex::new(network_client)),
            ack,
            canonicalization: None,
            clock: Arc::new(crate::clock::test::MockClock::default()),
        }
    }

    #[tokio::test]
    async fn test_configure_auth_success() {
        let account_id_hex = "0x069cde0ebf59f29063051ad8a3d32d";
        let (_account_id, pubkey_hex, signature_hex) = generate_falcon_signature(account_id_hex);

        let existing_metadata = AccountMetadata {
            account_id: account_id_hex.to_string(),
            auth: Auth::MidenFalconRpo {
                cosigner_pubkeys: vec![pubkey_hex.clone()],
            },
            storage_type: StorageType::Filesystem,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        };

        let state_json = serde_json::json!({
            "account_id": account_id_hex,
            "data": "mock_data"
        });

        let state_object = StateObject {
            account_id: account_id_hex.to_string(),
            state_json,
            commitment: "0x1234".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        };

        // Skip the pubkey validation step by not calling validate_pubkeys_match_storage
        // This is a simplified unit test focusing on auth flow, not storage validation
        let network_client = MockNetworkClient::new().with_validate_credential(Ok(()));
        let storage_backend = MockStorageBackend::new().with_pull_state(Ok(state_object));
        let metadata_store = MockMetadataStore::new()
            .with_get(Ok(Some(existing_metadata)))
            .with_update_auth(Ok(()));

        let state = create_test_app_state(network_client, storage_backend, metadata_store);

        let credential = Credentials::signature(pubkey_hex.clone(), signature_hex);

        // Test successful auth update with matching pubkey
        let params = ConfigureAuthParams {
            account_id: account_id_hex.to_string(),
            auth: Auth::MidenFalconRpo {
                cosigner_pubkeys: vec![pubkey_hex],
            },
            credential,
        };

        // Should succeed since validation is mocked to return Ok
        let result = configure_auth(&state, params).await;

        assert!(
            result.is_ok(),
            "Failed to configure auth: {:?}",
            result.err()
        );
        let result = result.unwrap();
        assert_eq!(result.account_id, account_id_hex);
        assert!(result.success);
    }

    #[tokio::test]
    async fn test_configure_auth_account_not_found() {
        let account_id_hex = "0x069cde0ebf59f29063051ad8a3d32d";
        let (_account_id, pubkey_hex, signature_hex) = generate_falcon_signature(account_id_hex);

        let network_client = MockNetworkClient::new();
        let storage_backend = MockStorageBackend::new();
        let metadata_store = MockMetadataStore::new().with_get(Ok(None));

        let state = create_test_app_state(network_client, storage_backend, metadata_store);

        let credential = Credentials::signature(pubkey_hex.clone(), signature_hex);

        let params = ConfigureAuthParams {
            account_id: account_id_hex.to_string(),
            auth: Auth::MidenFalconRpo {
                cosigner_pubkeys: vec![pubkey_hex],
            },
            credential,
        };

        let result = configure_auth(&state, params).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            PsmError::AccountNotFound(_) => {}
            e => panic!("Expected AccountNotFound error, got: {:?}", e),
        }
    }
}
