pub mod miden_falcon_rpo;

use crate::error::PsmError;
use miden_objects::crypto::dsa::rpo_falcon512::{SecretKey, Signature};
use miden_objects::Word;
use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, PsmError>;

#[derive(Debug, Clone)]
pub enum KeystoreConfig {
    Filesystem(PathBuf),
}

/// Main signer - provides cryptographic signing operations
#[derive(Clone)]
pub enum Signer {
    MidenFalconRpo(miden_falcon_rpo::MidenFalconRpoSigner),
}

impl Signer {
    /// Create a new Miden Falcon RPO signer with filesystem keystore
    pub fn miden_falcon_rpo(keystore: KeystoreConfig) -> Result<Self> {
        match keystore {
            KeystoreConfig::Filesystem(path) => {
                Ok(Signer::MidenFalconRpo(miden_falcon_rpo::MidenFalconRpoSigner::new(path)?))
            }
        }
    }

    /// Sign a message with the server's signing key
    pub fn sign_with_server_key(&self, message: Word) -> Result<Signature> {
        match self {
            Signer::MidenFalconRpo(signer) => signer.sign_with_server_key(message),
        }
    }

    /// Get the server's public key
    pub fn server_pubkey(&self) -> Word {
        match self {
            Signer::MidenFalconRpo(signer) => signer.server_pubkey(),
        }
    }

    /// Add a key to the keystore
    pub fn add_key(&self, key: &SecretKey) -> Result<()> {
        match self {
            Signer::MidenFalconRpo(signer) => signer.add_key(key),
        }
    }

    /// Get a key from the keystore by its public key
    pub fn get_key(&self, pub_key: Word) -> Result<SecretKey> {
        match self {
            Signer::MidenFalconRpo(signer) => signer.get_key(pub_key),
        }
    }

    /// Sign a message with a specific key from the keystore
    pub fn sign(&self, pub_key: Word, message: Word) -> Result<Signature> {
        match self {
            Signer::MidenFalconRpo(signer) => signer.sign(pub_key, message),
        }
    }
}
