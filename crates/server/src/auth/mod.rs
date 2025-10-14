mod miden_falcon_rpo;

use crate::metadata::AccountMetadata;

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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum Auth {
    /// Miden Falcon RPO signature scheme
    MidenFalconRpo,
}

impl Auth {
    /// Verify credentials are authorized for account
    ///
    /// # Arguments
    /// * `account_id` - The account ID
    /// * `credentials` - The credentials to verify
    /// * `account_metadata` - The account metadata containing authorization info
    pub fn verify(
        &self,
        account_id: &str,
        credentials: &Credentials,
        account_metadata: &AccountMetadata,
    ) -> Result<(), String> {
        match self {
            Auth::MidenFalconRpo => {
                let (pubkey, signature) = credentials
                    .as_signature()
                    .ok_or_else(|| "MidenFalconRpo requires signature credentials".to_string())?;

                if !account_metadata
                    .cosigner_pubkeys
                    .contains(&pubkey.to_string())
                {
                    return Err(format!("Public key '{pubkey}' is not authorized"));
                }

                miden_falcon_rpo::verify_request_signature(account_id, pubkey, signature)
            }
        }
    }
}
