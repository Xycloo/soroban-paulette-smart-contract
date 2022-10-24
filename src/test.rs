#![cfg(test)]

use crate::auction;
use crate::testutils::{register_test_contract as register_paulette, PauletteContract};
use crate::token::{self, TokenMetadata};
use rand::{thread_rng, RngCore};
use soroban_auth::{Identifier, Signature};
use soroban_sdk::bigint;
use soroban_sdk::{
    testutils::{Accounts, Ledger, LedgerInfo},
    AccountId, BigInt, BytesN, Env, IntoVal,
};

fn generate_contract_id() -> [u8; 32] {
    let mut id: [u8; 32] = Default::default();
    thread_rng().fill_bytes(&mut id);
    id
}

fn generate_office_id() -> [u8; 16] {
    let mut id: [u8; 16] = Default::default();
    thread_rng().fill_bytes(&mut id);
    id
}

fn create_token_contract(e: &Env, admin: &AccountId) -> ([u8; 32], token::Client) {
    let id = e.register_contract_token(None);
    let token = token::Client::new(e, &id);
    // decimals, name, symbol don't matter in tests
    token.init(
        &Identifier::Account(admin.clone()),
        &TokenMetadata {
            name: "USD coin".into_val(e),
            symbol: "USDC".into_val(e),
            decimals: 7,
        },
    );
    (id.into(), token)
}

fn create_paulette_contract(
    e: &Env,
    admin: &AccountId,
    token_id: &[u8; 32],
    tax: BigInt,
) -> ([u8; 32], PauletteContract) {
    let id = generate_contract_id();
    register_paulette(e, &id);
    let paulette = PauletteContract::new(e, &id);
    paulette.initialize(&Identifier::Account(admin.clone()), token_id, tax);
    (id, paulette)
}

#[test]
fn test_sequence() {
    let e: Env = Default::default();
    let admin1 = e.accounts().generate(); // generating the usdc admin

    let user1 = e.accounts().generate();
    let user2 = e.accounts().generate();
    let user1_id = Identifier::Account(user1.clone());
    let user2_id = Identifier::Account(user2.clone());

    let (contract1, usdc_token) = create_token_contract(&e, &admin1); // registered and initialized the usdc token contract
    let (contract_paulette, paulette) =
        create_paulette_contract(&e, &user1, &contract1, bigint!(&e, 20)); // registered and initialized the paulette token contract, with usdc as paulette token
    let paulette_id = Identifier::Contract(BytesN::from_array(&e, &contract_paulette)); // the id of the paulette

    let auction_id = BytesN::from_array(&e, &generate_contract_id());
    let auction_contract_id = Identifier::Contract(auction_id.clone());
    e.register_contract_wasm(&auction_id, auction::WASM);

    // minting 1000 usdc to user1
    usdc_token.with_source_account(&admin1).mint(
        &Signature::Invoker,
        &BigInt::zero(&e),
        &user1_id,
        &BigInt::from_u32(&e, 1000),
    );

    // minting 1000 usdc to user2
    usdc_token.with_source_account(&admin1).mint(
        &Signature::Invoker,
        &BigInt::zero(&e),
        &user2_id,
        &BigInt::from_u32(&e, 1000),
    );

    // setting ledger time to a recent timestamp
    e.ledger().set(LedgerInfo {
        timestamp: 1666359075,
        protocol_version: 1,
        sequence_number: 10,
        network_passphrase: Default::default(),
        base_reserve: 10,
    });

    let office_id = BytesN::from_array(&e, &generate_office_id());
    paulette.new_office(
        user1.clone(),
        office_id.clone(),
        auction_id,
        bigint!(&e, 5),
        bigint!(&e, 1),
        bigint!(&e, 900),
    );

    e.ledger().set(LedgerInfo {
        timestamp: 1666360875,
        protocol_version: 1,
        sequence_number: 10,
        network_passphrase: Default::default(),
        base_reserve: 10,
    });

    assert_eq!(paulette.get_price(office_id.clone()), 3);

    // user 1 deposits 5 usdc into paulette
    usdc_token.with_source_account(&user2).approve(
        &Signature::Invoker,
        &BigInt::zero(&e),
        &auction_contract_id,
        &paulette.get_price(office_id.clone()),
    );

    paulette.buy(office_id.clone(), user2_id.clone());

    assert_eq!(usdc_token.balance(&user1_id), 1003);

    e.ledger().set(LedgerInfo {
        timestamp: 1666965674,
        protocol_version: 1,
        sequence_number: 10,
        network_passphrase: Default::default(),
        base_reserve: 10,
    });

    usdc_token.with_source_account(&user2).approve(
        &Signature::Invoker,
        &BigInt::zero(&e),
        &paulette_id,
        &bigint!(&e, 20),
    );

    paulette.pay_tax(office_id.clone(), user2_id);
    assert_eq!(usdc_token.balance(&user1_id), 1023);

    e.ledger().set(LedgerInfo {
        timestamp: 1667570476,
        protocol_version: 1,
        sequence_number: 10,
        network_passphrase: Default::default(),
        base_reserve: 10,
    });

    let auction_1_id = BytesN::from_array(&e, &generate_contract_id());
    let _auction_1_contract_id = Identifier::Contract(auction_1_id.clone());
    e.register_contract_wasm(&auction_1_id, auction::WASM);

    paulette.revoke(
        user1,
        office_id.clone(),
        auction_1_id,
        bigint!(&e, 50),
        bigint!(&e, 5),
        bigint!(&e, 1800),
    );

    assert_eq!(paulette.get_price(office_id), 50);
}

#[test]
#[should_panic]
fn test_invalid_revoke() {
    let e: Env = Default::default();
    let admin1 = e.accounts().generate(); // generating the usdc admin

    let user1 = e.accounts().generate();
    let user2 = e.accounts().generate();
    let user1_id = Identifier::Account(user1.clone());
    let user2_id = Identifier::Account(user2.clone());

    let (contract1, usdc_token) = create_token_contract(&e, &admin1); // registered and initialized the usdc token contract
    let (_contract_paulette, paulette) =
        create_paulette_contract(&e, &user1, &contract1, bigint!(&e, 20)); // registered and initialized the paulette token contract, with usdc as paulette token

    let auction_id = BytesN::from_array(&e, &generate_contract_id());
    let auction_contract_id = Identifier::Contract(auction_id.clone());
    e.register_contract_wasm(&auction_id, auction::WASM);

    // minting 1000 usdc to user1
    usdc_token.with_source_account(&admin1).mint(
        &Signature::Invoker,
        &BigInt::zero(&e),
        &user1_id,
        &BigInt::from_u32(&e, 1000),
    );

    // minting 1000 usdc to user2
    usdc_token.with_source_account(&admin1).mint(
        &Signature::Invoker,
        &BigInt::zero(&e),
        &user2_id,
        &BigInt::from_u32(&e, 1000),
    );

    // setting ledger time to a recent timestamp
    e.ledger().set(LedgerInfo {
        timestamp: 1666359075,
        protocol_version: 1,
        sequence_number: 10,
        network_passphrase: Default::default(),
        base_reserve: 10,
    });

    let office_id = BytesN::from_array(&e, &generate_office_id());
    paulette.new_office(
        user1.clone(),
        office_id.clone(),
        auction_id.clone(),
        bigint!(&e, 5),
        bigint!(&e, 1),
        bigint!(&e, 900),
    );

    e.ledger().set(LedgerInfo {
        timestamp: 1666360875,
        protocol_version: 1,
        sequence_number: 10,
        network_passphrase: Default::default(),
        base_reserve: 10,
    });

    assert_eq!(paulette.get_price(office_id.clone()), 3);

    // user 1 deposits 5 usdc into paulette
    usdc_token.with_source_account(&user2).approve(
        &Signature::Invoker,
        &BigInt::zero(&e),
        &auction_contract_id,
        &paulette.get_price(office_id.clone()),
    );

    paulette.buy(office_id.clone(), user2_id);

    assert_eq!(usdc_token.balance(&user1_id), 1003);

    e.ledger().set(LedgerInfo {
        timestamp: 1666965674,
        protocol_version: 1,
        sequence_number: 10,
        network_passphrase: Default::default(),
        base_reserve: 10,
    });

    // shouldn't be able to revoke since office hasn't expired yet
    paulette.revoke(
        user1,
        office_id,
        auction_id,
        bigint!(&e, 1),
        bigint!(&e, 1),
        bigint!(&e, 1),
    );
}

#[test]
#[should_panic]
fn test_invalid_admin() {
    let e: Env = Default::default();
    let admin1 = e.accounts().generate(); // generating the usdc admin

    let user1 = e.accounts().generate();
    let user2 = e.accounts().generate();
    let user1_id = Identifier::Account(user1.clone());
    let user2_id = Identifier::Account(user2.clone());

    let (contract1, usdc_token) = create_token_contract(&e, &admin1); // registered and initialized the usdc token contract
    let (_contract_paulette, paulette) =
        create_paulette_contract(&e, &user1, &contract1, bigint!(&e, 20)); // registered and initialized the paulette token contract, with usdc as paulette token
    let auction_id = BytesN::from_array(&e, &generate_contract_id());

    // minting 1000 usdc to user1
    usdc_token.with_source_account(&admin1).mint(
        &Signature::Invoker,
        &BigInt::zero(&e),
        &user1_id,
        &BigInt::from_u32(&e, 1000),
    );

    // minting 1000 usdc to user2
    usdc_token.with_source_account(&admin1).mint(
        &Signature::Invoker,
        &BigInt::zero(&e),
        &user2_id,
        &BigInt::from_u32(&e, 1000),
    );

    // setting ledger time to a recent timestamp
    e.ledger().set(LedgerInfo {
        timestamp: 1666359075,
        protocol_version: 1,
        sequence_number: 10,
        network_passphrase: Default::default(),
        base_reserve: 10,
    });

    let office_id = BytesN::from_array(&e, &generate_office_id());
    paulette.new_office(
        user2, // not the admin
        office_id,
        auction_id,
        bigint!(&e, 5),
        bigint!(&e, 1),
        bigint!(&e, 900),
    );
}
