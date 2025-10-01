mod account;
mod masm;

use account::{FromJson, ToJson};
use miden_lib::account::{auth::AuthRpoFalcon512, wallets::BasicWallet};
use miden_objects::{
    account::AccountBuilder,
    crypto::{dsa::rpo_falcon512::PublicKey, rand::Randomizable},
};
use miden_vm::Word;

fn main() {
    let public_key = PublicKey::new(Word::from_random_bytes(&[0; 32]).unwrap());

    let (account, _) = AccountBuilder::new([0xff; 32])
        .with_auth_component(AuthRpoFalcon512::new(public_key))
        .with_component(BasicWallet)
        .build()
        .unwrap();

    let json = account.to_json();
    println!("{:?}", json);
    let account = miden_objects::account::Account::from_json(&json).unwrap();

    let json = account.to_json();
    println!("{:?}", json);
}
