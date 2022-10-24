#![cfg(any(test, feature = "testutils"))]

use crate::{Auth, PauletteContractClient};
use soroban_auth::Identifier;

use soroban_sdk::{AccountId, BigInt, BytesN, Env};

pub fn register_test_contract(e: &Env, contract_id: &[u8; 32]) {
    let contract_id = BytesN::from_array(e, contract_id);
    e.register_contract(&contract_id, crate::PauletteContract {});
}

pub struct PauletteContract {
    env: Env,
    contract_id: BytesN<32>,
}

impl PauletteContract {
    fn client(&self) -> PauletteContractClient {
        PauletteContractClient::new(&self.env, &self.contract_id)
    }

    pub fn new(env: &Env, contract_id: &[u8; 32]) -> Self {
        Self {
            env: env.clone(),
            contract_id: BytesN::from_array(env, contract_id),
        }
    }

    pub fn initialize(&self, admin: &Identifier, token_id: &[u8; 32], tax: BigInt) {
        self.client()
            .initialize(admin, &BytesN::from_array(&self.env, token_id), &tax);
    }

    pub fn nonce(&self) -> BigInt {
        self.client().nonce()
    }

    pub fn get_price(&self, id: BytesN<16>) -> BigInt {
        self.client().get_price(&id)
    }

    pub fn new_office(
        &self,
        admin: AccountId,
        id: BytesN<16>,
        auction: BytesN<32>,
        price: BigInt,
        min_price: BigInt,
        slope: BigInt,
    ) {
        self.env.set_source_account(&admin);
        self.client().new_office(
            &Auth {
                sig: soroban_auth::Signature::Invoker,
                nonce: BigInt::zero(&self.env),
            },
            &id,
            &auction,
            &price,
            &min_price,
            &slope,
        )
    }

    pub fn buy(&self, id: BytesN<16>, buyer: Identifier) {
        self.client().buy(&id, &buyer);
    }

    pub fn pay_tax(&self, id: BytesN<16>, payer: Identifier) {
        self.client().pay_tax(&id, &payer)
    }

    pub fn revoke(
        &self,
        admin: AccountId,
        id: BytesN<16>,
        auction: BytesN<32>,
        price: BigInt,
        min_price: BigInt,
        slope: BigInt,
    ) {
        self.env.set_source_account(&admin);
        self.client().revoke(
            &Auth {
                sig: soroban_auth::Signature::Invoker,
                nonce: BigInt::zero(&self.env),
            },
            &id,
            &auction,
            &price,
            &min_price,
            &slope,
        )
    }

    /*
    TODO: revoke testutil
    pub fn revoke(&self)  {
            self.client().revoke(id, auction, price, min_price, slope)
        }
    */
}
