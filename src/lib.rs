#![no_std]
#![warn(
    unused,
    future_incompatible,
    nonstandard_style,
    rust_2018_idioms,
    missing_docs
)]

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

// Perform arithmetic ops on custom types
trait Arithmetic<Rhs = Self> {
    type Output;

    fn add(self, rhs: Rhs) -> Self::Output;
}

#[derive(Clone)]
#[contracttype]
/// Keys for the contract data
pub enum DataKey {
    /// What standard token to use in the contract
    TokenId,
    /// Contract admin
    Admin,
    /// Tax to pay to keep the office after a week
    Tax,
    /// Key for offices that are for sale
    ForSale(BytesN<16>),
    /// Key for offices that have been bought
    Bought(BytesN<16>),
    /// Admin nonce
    Nonce(Identifier),
}

#[derive(Clone)]
#[contracttype]
/// Auth type to wrap admin signature and nonce together
pub struct Auth {
    pub sig: Signature,
    pub nonce: BigInt,
}

#[derive(Clone, PartialEq, PartialOrd, Eq, Ord, Debug)]
#[contracttype]
/// Timestamp type to enforce explicitness
pub struct TimeStamp(pub u64);

impl Arithmetic<TimeStamp> for TimeStamp {
    type Output = TimeStamp;

    fn add(self, other: Self) -> Self {
        Self(self.0 + other.0)
    }
}

#[derive(Clone)]
#[contracttype]
/// Office struct, stored with key DataKey::Bought(id)
pub struct Office {
    pub user: Identifier,
    pub expires: TimeStamp,
}

fn new_auction(e: &Env, id: BytesN<32>, price: BigInt, min_price: BigInt, slope: BigInt) {
    let client = auction::Client::new(e, id);
    client.initialize(
        &read_administrator(e),
        &get_token_id(e),
        &price,
        &min_price,
        &slope,
    );
}

fn bid_auction(e: &Env, id: BytesN<32>, buyer: Identifier) -> bool {
    let client = auction::Client::new(e, id);
    client.buy(&buyer)
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

fn remove_for_sale(e: &Env, id: BytesN<16>) {
    let key = DataKey::ForSale(id);
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

fn put_token_id(e: &Env, token_id: BytesN<32>) {
    let key = DataKey::TokenId;
    e.data().set(key, token_id);
}

fn put_tax(e: &Env, amount: BigInt) {
    let key = DataKey::Tax;
    e.data().set(key, amount);
}

fn get_tax(e: &Env) -> BigInt {
    let key = DataKey::Tax;
    e.data().get(key).unwrap().unwrap()
}

fn get_token_id(e: &Env) -> BytesN<32> {
    let key = DataKey::TokenId;
    e.data().get(key).unwrap().unwrap()
}

fn transfer_in_vault(e: &Env, from: Identifier, amount: BigInt) {
    let client = token::Client::new(e, get_token_id(e));

    client.xfer_from(
        &Signature::Invoker,
        &BigInt::zero(e),
        &from,
        &read_administrator(e),
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

fn check_admin(e: &Env, auth: &Signature) {
    let auth_id = auth.identifier(e);
    if auth_id != read_administrator(e) {
        panic!("not authorized by admin")
    }
}

fn read_nonce(e: &Env, id: &Identifier) -> BigInt {
    let key = DataKey::Nonce(id.clone());
    e.data()
        .get(key)
        .unwrap_or_else(|| Ok(BigInt::zero(e)))
        .unwrap()
}

fn verify_and_consume_nonce(e: &Env, auth: &Signature, expected_nonce: &BigInt) {
    match auth {
        Signature::Invoker => {
            if BigInt::zero(&e) != expected_nonce {
                panic!("nonce should be zero for Invoker")
            }
            return;
        }
        _ => {}
    }

    let id = auth.identifier(&e);
    let key = DataKey::Nonce(id.clone());
    let nonce = read_nonce(e, &id);

    if nonce != expected_nonce {
        panic!("incorrect nonce")
    }
    e.data().set(key, &nonce + 1);
}

fn make_new_office(
    e: &Env,
    id: BytesN<16>,
    auction: BytesN<32>,
    price: BigInt,
    min_price: BigInt,
    slope: BigInt,
) {
    new_auction(e, auction.clone(), price, min_price, slope);
    put_for_sale(e, id, auction);
}

fn get_office_price(e: &Env, id: BytesN<16>) -> BigInt {
    let auction_id = get_for_sale(e, id);
    let client = auction::Client::new(e, auction_id);

    client.get_price()
}

pub trait PauletteContractTrait {
    /// Sets the admin and the Royal vault's token id
    fn initialize(e: Env, admin: Identifier, token_id: BytesN<32>, tax: BigInt);

    /// Returns the nonce for the admin
    fn nonce(e: Env) -> BigInt;

    /// Call to buy an office
    fn buy(e: Env, id: BytesN<16>, buyer: Identifier);

    /// Call to pay taxes for a given office
    fn pay_tax(e: Env, id: BytesN<16>, payer: Identifier);

    /// Query the price of a given office
    fn get_price(e: Env, id: BytesN<16>) -> BigInt;

    /// Create a new office (requires admin auth)
    fn new_office(
        e: Env,
        admin: Auth,
        id: BytesN<16>,
        auction: BytesN<32>,
        price: BigInt,
        min_price: BigInt,
        slope: BigInt,
    );

    /// remove office from Bought, add it to ForSale, create new dutch auction contract with the given ID
    fn revoke(
        e: Env,
        admin: Auth,
        id: BytesN<16>,
        auction: BytesN<32>,
        price: BigInt,
        min_price: BigInt,
        slope: BigInt,
    );
}

pub struct PauletteContract;

#[contractimpl]
impl PauletteContractTrait for PauletteContract {
    fn initialize(e: Env, admin: Identifier, token_id: BytesN<32>, tax: BigInt) {
        if has_administrator(&e) {
            panic!("admin is already set");
        }

        write_administrator(&e, admin);
        put_token_id(&e, token_id);
        put_tax(&e, tax);
    }

    fn nonce(e: Env) -> BigInt {
        read_nonce(&e, &read_administrator(&e))
    }

    fn buy(e: Env, id: BytesN<16>, buyer: Identifier) {
        let auction_id = get_for_sale(&e, id.clone());
        let auction_result = bid_auction(&e, auction_id, buyer.clone());

        // explicit handle
        if !auction_result {
            panic!("bidding failed")
        }

        remove_for_sale(&e, id.clone());
        put_bought(
            &e,
            id,
            Office {
                user: buyer,
                expires: get_ts(&e).add(TimeStamp(604800)),
            },
        )
    }

    // the contract doesn't care if its the user who pays the office, just that someone is.
    fn pay_tax(e: Env, id: BytesN<16>, payer: Identifier) {
        transfer_in_vault(&e, payer, get_tax(&e));
        let mut office = get_bought(&e, id.clone());

        // dilemma: allow to pay taxes even after they have expired if the admin doesn't revoke the office?
        office.expires = office.expires.add(TimeStamp(604800));

        put_bought(&e, id, office);
    }

    fn new_office(
        e: Env,
        admin: Auth,
        id: BytesN<16>,
        auction: BytesN<32>,
        price: BigInt,
        min_price: BigInt,
        slope: BigInt,
    ) {
        check_admin(&e, &admin.sig);
        verify_and_consume_nonce(&e, &admin.sig, &admin.nonce);

        if e.data().has(DataKey::ForSale(id.clone())) {
            panic!("id already exists")
        }

        if e.data().has(DataKey::Bought(id.clone())) {
            panic!("id already exists")
        }

        make_new_office(&e, id, auction, price, min_price, slope);
    }

    fn get_price(e: Env, id: BytesN<16>) -> BigInt {
        get_office_price(&e, id)
    }

    fn revoke(
        e: Env,
        admin: Auth,
        id: BytesN<16>,
        auction: BytesN<32>,
        price: BigInt,
        min_price: BigInt,
        slope: BigInt,
    ) {
        check_admin(&e, &admin.sig);
        verify_and_consume_nonce(&e, &admin.sig, &admin.nonce);

        let office = get_bought(&e, id.clone());

        if office.expires > get_ts(&e) {
            panic!("office is not expired yet");
        }

        remove_bought(&e, id.clone());
        make_new_office(&e, id, auction, price, min_price, slope);
    }
}
