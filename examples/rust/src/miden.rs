use miden_client::{Client, Felt, Word, auth::Signature, crypto::RpoRandomCoin, keystore::FilesystemKeyStore};
use rand_chacha::ChaCha20Rng;


pub async fn create_client(
    keystore: &FilesystemKeyStore<ChaCha20Rng>,
) -> () {

}
