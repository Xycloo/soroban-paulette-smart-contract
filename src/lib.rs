#![no_std]

#[cfg(feature = "testutils")]
extern crate std;

mod test;
pub mod testutils;

use soroban_auth::{Identifier, Signature};
use soroban_sdk::{contractimpl, contracttype, BigInt, BytesN, Env};

mod token {
    soroban_sdk::contractimport!(file = "./soroban_token_spec.wasm");
}

mod auction {
    use crate::{Identifier, Signature};
    soroban_sdk::contractimport!(file = "./soroban_dutch_auction_contract.wasm");
}

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    TokenId,
    Admin,
    ForSale(BytesN<16>),
    Bought(BytesN<16>),
    Balance(Identifier),
    Nonce(Identifier),
}

#[derive(Clone)]
#[contracttype]
pub struct Auth {
    pub sig: Signature,
    pub nonce: BigInt,
}

#[derive(Clone, PartialEq, PartialOrd, Eq, Ord)]
#[contracttype]
pub struct TimeStamp(pub u64);

#[derive(Clone)]
#[contracttype]
pub struct Office {
    pub user: Identifier,
    pub expires: TimeStamp,
    pub latest: TimeStamp,
    pub interval: u64,
    pub auction: BytesN<32>,
}

fn get_contract_id(e: &Env) -> Identifier {
    Identifier::Contract(e.get_current_contract())
}

fn new_auction(e: &Env, id: BytesN<32>, price: BigInt, min_price: BigInt, slope: BigInt) {
    let client = auction::Client::new(e, id);
    client.initialize(
        &get_contract_id(e),
        &get_token_id(e),
        &price,
        &min_price,
        &slope,
    );
}

fn get_ts(e: &Env) -> TimeStamp {
    TimeStamp(e.ledger().timestamp())
}

fn put_bought(e: &Env, id: BytesN<16>, bought: Office) {
    let key = DataKey::Bought(id);
    e.data().set(key, bought);
}

fn get_bought(e: &Env, id: BytesN<16>) -> Office {
    let key = DataKey::Bought(id);
    e.data().get(key).unwrap().unwrap()
}

fn remove_bought(e: &Env, id: BytesN<16>) {
    let key = DataKey::Bought(id);
    e.data().remove(key);
}

fn put_for_sale(e: &Env, id: BytesN<16>, auction: BytesN<32>) {
    let key = DataKey::ForSale(id);
    e.data().set(key, auction)
}

fn get_for_sale(e: &Env, id: BytesN<16>) -> BytesN<32> {
    let key = DataKey::ForSale(id);
    e.data().get(key).unwrap().unwrap()
}

fn put_id_balance(e: &Env, id: Identifier, amount: BigInt) {
    let key = DataKey::Balance(id);
    e.data().set(key, amount);
}

fn get_id_balance(e: &Env, id: Identifier) -> BigInt {
    let key = DataKey::Balance(id);
    e.data().get(key).unwrap_or(Ok(BigInt::zero(&e))).unwrap()
}

fn put_token_id(e: &Env, token_id: BytesN<32>) {
    let key = DataKey::TokenId;
    e.data().set(key, token_id);
}

fn get_token_id(e: &Env) -> BytesN<32> {
    let key = DataKey::TokenId;
    e.data().get(key).unwrap().unwrap()
}

fn get_token_balance(e: &Env) -> BigInt {
    let contract_id = get_token_id(e);
    token::Client::new(e, contract_id).balance(&get_contract_id(e))
}

fn transfer(e: &Env, to: Identifier, amount: BigInt) {
    let client = token::Client::new(e, get_token_id(e));
    client.xfer(
        &Signature::Invoker,
        &client.nonce(&Signature::Invoker.identifier(e)),
        &to,
        &amount,
    );
}

fn transfer_in_vault(e: &Env, from: Identifier, amount: BigInt) {
    let client = token::Client::new(e, get_token_id(e));
    let vault_id = get_contract_id(e);

    client.xfer_from(
        &Signature::Invoker,
        &BigInt::zero(&e),
        &from,
        &vault_id,
        &amount,
    )
}

fn has_administrator(e: &Env) -> bool {
    let key = DataKey::Admin;
    e.data().has(key)
}

fn read_administrator(e: &Env) -> Identifier {
    let key = DataKey::Admin;
    e.data().get_unchecked(key).unwrap()
}

fn write_administrator(e: &Env, id: Identifier) {
    let key = DataKey::Admin;
    e.data().set(key, id);
}

fn read_nonce(e: &Env, id: &Identifier) -> BigInt {
    let key = DataKey::Nonce(id.clone());
    e.data()
        .get(key)
        .unwrap_or_else(|| Ok(BigInt::zero(e)))
        .unwrap()
}

pub trait VaultContractTrait {
    // Sets the admin and the vault's token id
    fn initialize(e: Env, admin: Identifier, token_id: BytesN<32>);

    // Returns the nonce for the admin
    fn nonce(e: Env) -> BigInt;

    fn buy(e: Env, id: BytesN<16>, buyer: Identifier);

    fn pay_tax(e: Env, id: BytesN<16>, payer: Identifier);

    /// remove office from Bought, add it to ForSale, create new dutch auction contract with the given ID
    fn revoke(
        e: Env,
        id: BytesN<16>,
        auction: BytesN<32>,
        price: BigInt,
        min_price: BigInt,
        slope: BigInt,
    );
}

pub struct VaultContract;

#[contractimpl]
impl VaultContractTrait for VaultContract {
    fn initialize(e: Env, admin: Identifier, token_id: BytesN<32>) {
        if has_administrator(&e) {
            panic!("admin is already set");
        }

        write_administrator(&e, admin);
        put_token_id(&e, token_id)
    }

    fn nonce(e: Env) -> BigInt {
        read_nonce(&e, &read_administrator(&e))
    }

    fn buy(e: Env, id: BytesN<16>, buyer: Identifier) {}

    // has payer since the contract doesn't care if its the user who pays the office, just that someone is.
    fn pay_tax(e: Env, id: BytesN<16>, payer: Identifier) {}

    fn revoke(
        e: Env,
        id: BytesN<16>,
        auction: BytesN<32>,
        price: BigInt,
        min_price: BigInt,
        slope: BigInt,
    ) {
        let office = get_bought(&e, id.clone());

        if office.expires < get_ts(&e) {
            panic!("office is not expired yet");
        }

        remove_bought(&e, id.clone());
        new_auction(&e, auction.clone(), price, min_price, slope);
        put_for_sale(&e, id, auction);
    }
}
