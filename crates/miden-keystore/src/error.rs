use std::fmt;

#[derive(Debug)]
pub enum KeyStoreError {
    StorageError(String),
    DecodingError(String),
}

impl fmt::Display for KeyStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeyStoreError::StorageError(msg) => write!(f, "Storage error: {msg}"),
            KeyStoreError::DecodingError(msg) => write!(f, "Decoding error: {msg}"),
        }
    }
}

impl std::error::Error for KeyStoreError {}

pub type Result<T> = std::result::Result<T, KeyStoreError>;
