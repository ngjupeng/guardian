use crate::api::grpc::state_manager::auth_config;
use crate::error::PsmError;
use crate::storage::MetadataStore;
use axum::{extract::FromRequestParts, http::request::Parts};

mod miden_falcon_rpo;

/// Trait for extracting authentication credentials from request metadata
/// Implemented by HTTP headers and gRPC metadata
pub trait ExtractCredentials {
    type Error;

    /// Extract credentials from the metadata source
    fn extract_credentials(&self) -> Result<Credentials, Self::Error>;
}

/// Authentication credentials enum - extensible for different auth methods
#[derive(Debug, Clone)]
pub enum Credentials {
    /// Public key signature-based authentication
    /// Used for cryptographic signature verification (e.g., Falcon, ECDSA, etc.)
    Signature { pubkey: String, signature: String },
}

impl Credentials {
    pub fn signature(pubkey: String, signature: String) -> Self {
        Self::Signature { pubkey, signature }
    }

    pub fn as_signature(&self) -> Option<(&str, &str)> {
        match self {
            Self::Signature { pubkey, signature } => Some((pubkey, signature)),
        }
    }
}

/// Authentication and authorization handler
/// Defines which signature scheme to use and handles verification
/// Each variant contains auth-specific authorization data
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum Auth {
    /// Miden Falcon RPO signature scheme
    /// Contains list of authorized cosigner public keys
    MidenFalconRpo { cosigner_pubkeys: Vec<String> },
}

impl Auth {
    /// Verify credentials are authorized for account
    ///
    /// # Arguments
    /// * `account_id` - The account ID
    /// * `credentials` - The credentials to verify
    pub fn verify(&self, account_id: &str, credentials: &Credentials) -> Result<(), String> {
        match self {
            Auth::MidenFalconRpo { cosigner_pubkeys } => {
                let (pubkey, signature) = credentials
                    .as_signature()
                    .ok_or_else(|| "MidenFalconRpo requires signature credentials".to_string())?;

                // Check authorization - pubkey must be in cosigner list
                if !cosigner_pubkeys.contains(&pubkey.to_string()) {
                    return Err(format!("Public key '{pubkey}' is not authorized"));
                }

                // Verify cryptographic signature
                miden_falcon_rpo::verify_request_signature(account_id, pubkey, signature)
            }
        }
    }
}

// Implementation for HTTP headers (axum::http::HeaderMap)
impl ExtractCredentials for axum::http::HeaderMap {
    type Error = String;

    fn extract_credentials(&self) -> Result<Credentials, Self::Error> {
        let pubkey = self
            .get("x-pubkey")
            .ok_or_else(|| "Missing x-pubkey header".to_string())?
            .to_str()
            .map_err(|_| "Invalid x-pubkey header".to_string())?
            .to_string();

        let signature = self
            .get("x-signature")
            .ok_or_else(|| "Missing x-signature header".to_string())?
            .to_str()
            .map_err(|_| "Invalid x-signature header".to_string())?
            .to_string();

        Ok(Credentials::signature(pubkey, signature))
    }
}

// Implementation for gRPC metadata (tonic::metadata::MetadataMap)
impl ExtractCredentials for tonic::metadata::MetadataMap {
    type Error = tonic::Status;

    fn extract_credentials(&self) -> Result<Credentials, Self::Error> {
        let pubkey = self
            .get("x-pubkey")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| tonic::Status::invalid_argument("Missing or invalid x-pubkey metadata"))?
            .to_string();

        let signature = self
            .get("x-signature")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| {
                tonic::Status::invalid_argument("Missing or invalid x-signature metadata")
            })?
            .to_string();

        Ok(Credentials::signature(pubkey, signature))
    }
}

// Conversion from gRPC proto AuthConfig to Auth enum
impl TryFrom<crate::api::grpc::state_manager::AuthConfig> for Auth {
    type Error = String;

    fn try_from(
        auth_config: crate::api::grpc::state_manager::AuthConfig,
    ) -> Result<Self, Self::Error> {
        match auth_config.auth_type {
            Some(auth_config::AuthType::MidenFalconRpo(miden_auth)) => Ok(Auth::MidenFalconRpo {
                cosigner_pubkeys: miden_auth.cosigner_pubkeys,
            }),
            None => Err("Auth type not specified".to_string()),
        }
    }
}

/// Typed HTTP auth extractor to remove header parsing duplication
pub struct AuthHeader(pub Credentials);

impl<S> FromRequestParts<S> for AuthHeader
where
    S: Send + Sync,
{
    type Rejection = PsmError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let creds = parts
            .headers
            .extract_credentials()
            .map_err(PsmError::AuthenticationFailed)?;
        Ok(AuthHeader(creds))
    }
}

pub async fn update_credentials(
    store: &dyn MetadataStore,
    account_id: &str,
    new_auth: Auth,
    now: &str,
) -> Result<(), PsmError> {
    let mut metadata = store
        .get(account_id)
        .await
        .map_err(|e| PsmError::StorageError(format!("Failed to get metadata: {e}")))?
        .ok_or_else(|| PsmError::AccountNotFound(account_id.to_string()))?;

    if metadata.auth == new_auth {
        return Ok(());
    }

    metadata.auth = new_auth;
    metadata.updated_at = now.to_string();

    store
        .set(metadata)
        .await
        .map_err(|e| PsmError::StorageError(format!("Failed to update metadata: {e}")))?;

    Ok(())
}
