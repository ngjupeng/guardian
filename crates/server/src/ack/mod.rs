pub mod miden_falcon_rpo;

use crate::error::Result;
use crate::storage::DeltaObject;

pub use miden_falcon_rpo::MidenFalconRpoSigner;

/// Acknowledger for server operations
///
/// Different acknowledgement methods can be used (signature-based, timestamp-based, etc.)
#[derive(Clone)]
pub enum Acknowledger {
    FilesystemMidenFalconRpo(MidenFalconRpoSigner),
}

impl Acknowledger {
    /// Get the server's public key as a hex string
    pub fn pubkey(&self) -> String {
        match self {
            Acknowledger::FilesystemMidenFalconRpo(signer) => signer.pubkey_hex(),
        }
    }

    /// Acknowledge a delta and return it with ack_sig loaded
    pub fn ack_delta(&self, delta: DeltaObject) -> Result<DeltaObject> {
        match self {
            Acknowledger::FilesystemMidenFalconRpo(signer) => signer.ack_delta(delta),
        }
    }
}
