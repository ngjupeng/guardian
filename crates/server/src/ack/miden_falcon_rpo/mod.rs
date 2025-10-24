mod signer;

pub use crate::error::MidenFalconRpoError;
pub use miden_keystore::FilesystemKeyStore;
pub use signer::MidenFalconRpoSigner;

pub type Result<T> = std::result::Result<T, MidenFalconRpoError>;
