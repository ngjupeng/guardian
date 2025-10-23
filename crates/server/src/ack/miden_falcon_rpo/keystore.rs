//! Filesystem-based keystore for Miden Falcon RPO.
//!
//! The code is based on https://github.com/0xMiden/miden-client/blob/next/crates/rust-client/src/keystore/fs_keystore.rs
use std::fs::{self, OpenOptions};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use miden_objects::Word;
use miden_objects::crypto::dsa::rpo_falcon512::{SecretKey, Signature};
use miden_objects::utils::{Deserializable, Serializable};
use rand::{Rng, SeedableRng};

use crate::error::MidenFalconRpoError;

type Result<T> = std::result::Result<T, MidenFalconRpoError>;

#[derive(Debug, Clone)]
pub struct FilesystemKeyStore<R: Rng + Send + Sync> {
    rng: Arc<RwLock<R>>,
    keys_directory: PathBuf,
}

impl<R: Rng + Send + Sync> FilesystemKeyStore<R> {
    pub fn with_rng(keys_directory: PathBuf, rng: R) -> Result<Self> {
        fs::create_dir_all(&keys_directory).map_err(|e| {
            MidenFalconRpoError::StorageError(format!("Failed to create keys directory: {e}"))
        })?;

        Ok(Self {
            rng: Arc::new(RwLock::new(rng)),
            keys_directory,
        })
    }

    pub fn add_key(&self, key: &SecretKey) -> Result<()> {
        let pub_key = key.public_key();
        let pub_key_word: Word = pub_key.into();
        let filename = hash_pub_key(pub_key_word);
        let file_path = self.keys_directory.join(&filename);

        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&file_path)
            .map_err(|e| {
                MidenFalconRpoError::StorageError(format!(
                    "Failed to open key file {filename}: {e}"
                ))
            })?;

        let mut writer = BufWriter::new(file);
        let key_bytes = key.to_bytes();
        let hex_encoded = hex::encode(key_bytes);

        writer.write_all(hex_encoded.as_bytes()).map_err(|e| {
            MidenFalconRpoError::StorageError(format!(
                "Failed to write key to file {filename}: {e}"
            ))
        })?;

        writer.flush().map_err(|e| {
            MidenFalconRpoError::StorageError(format!("Failed to flush key file {filename}: {e}"))
        })?;

        Ok(())
    }

    pub fn get_key(&self, pub_key: Word) -> Result<SecretKey> {
        let filename = hash_pub_key(pub_key);
        let file_path = self.keys_directory.join(&filename);

        let file = OpenOptions::new()
            .read(true)
            .open(&file_path)
            .map_err(|e| {
                MidenFalconRpoError::StorageError(format!(
                    "Failed to open key file {filename}: {e}"
                ))
            })?;

        let mut reader = BufReader::new(file);
        let mut hex_encoded = String::new();

        reader.read_line(&mut hex_encoded).map_err(|e| {
            MidenFalconRpoError::StorageError(format!(
                "Failed to read key from file {filename}: {e}"
            ))
        })?;

        let key_bytes = hex::decode(hex_encoded.trim()).map_err(|e| {
            MidenFalconRpoError::DecodingError(format!(
                "Failed to decode hex key from file {filename}: {e}"
            ))
        })?;

        SecretKey::read_from_bytes(&key_bytes).map_err(|e| {
            MidenFalconRpoError::DecodingError(format!(
                "Failed to deserialize key from file {filename}: {e}"
            ))
        })
    }

    pub fn sign(&self, pub_key: Word, message: Word) -> Result<Signature> {
        let secret_key = self.get_key(pub_key)?;
        let mut rng_guard = self.rng.write().unwrap();
        Ok(secret_key.sign_with_rng::<R>(message, &mut *rng_guard))
    }

    pub fn generate_key(&self) -> Result<Word> {
        let secret_key = SecretKey::new();
        let pub_key: Word = secret_key.public_key().into();

        self.add_key(&secret_key)?;

        Ok(pub_key)
    }
}

impl<R: Rng + SeedableRng + Send + Sync> FilesystemKeyStore<R> {
    pub fn new(keys_directory: PathBuf) -> Result<Self> {
        let rng = R::seed_from_u64(rand::random());
        Self::with_rng(keys_directory, rng)
    }
}

fn hash_pub_key(pub_key: Word) -> String {
    let mut hasher = DefaultHasher::new();
    pub_key.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand_chacha::ChaCha20Rng;

    #[test]
    fn test_add_and_get_key() {
        let temp_dir = std::env::temp_dir().join(format!("keystore_test_{}", uuid::Uuid::new_v4()));
        let keystore = FilesystemKeyStore::<ChaCha20Rng>::new(temp_dir.clone()).unwrap();

        let secret_key = SecretKey::new();
        let pub_key: Word = secret_key.public_key().into();

        keystore.add_key(&secret_key).unwrap();
        let retrieved_key = keystore.get_key(pub_key).unwrap();

        assert_eq!(secret_key.to_bytes(), retrieved_key.to_bytes());

        std::fs::remove_dir_all(temp_dir).ok();
    }

    #[test]
    fn test_hash_pub_key() {
        use miden_objects::Felt;
        let pub_key = [Felt::new(1), Felt::new(2), Felt::new(3), Felt::new(4)];
        let hash1 = hash_pub_key(pub_key.into());
        let hash2 = hash_pub_key(pub_key.into());

        assert_eq!(hash1, hash2);
        assert!(!hash1.is_empty());
    }
}
