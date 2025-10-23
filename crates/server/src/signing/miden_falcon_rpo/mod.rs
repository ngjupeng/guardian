mod keystore;
mod signer;

pub use crate::error::MidenFalconRpoError;
pub use keystore::FilesystemKeyStore;
pub use signer::MidenFalconRpoSigner;

pub type Result<T> = std::result::Result<T, MidenFalconRpoError>;
