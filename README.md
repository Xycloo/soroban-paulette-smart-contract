# Soroban Offices Smart Contract

### Inspiration
The Paulette, was a law enforced by France in the beginning of the 17th century. It allowed the king to sell the ownership of public offices (making them hereditary). The owners of these public offices had to pay a tax each $\Delta t$ to keep a bought public office. These offices expire every $\Delta t$ and the owner of each office has to pay a tax to maintain the ownership, if they didn't pay this tax, the office could be revoked by the king.

## Actual implementation Abstract
The contract admin can create new offices and put them for sale by invoking the [Soroban Dutch Auction Contract](https://github.com/Xycloo/soroban-dutch-auction-contract). This means that these offices start with a certain price which decreases with time. Users can then buy these offices at the auction's price and become the owners. The offices expire after a week ( $604800s$ ), and can be renewed by paying a tax (ideally less than the price at which the office was bought). If more than a week goes by and the owner hasn't paid the tax yet, the admin can revoke the office and put it for sale in an auction again. 


## What You'll Learn
This contract is quite complete in terms of used functionalities, by looking at the code and following this post you'll learn:
- how Soroban's contract data storage works for inserting, getting and deleting data.
- how to call a contract from within another contract by calling an external contract for the auction (and one for the standard token implementation).
- how to use your custom types for more explicitness in your code (in this contract only the `TimeStamp` type for simplicity, but on a production-level contract you may have to explicit most of the types that can become ambiguous to a future maintainer).
- using the standard token implementation to transfer tokens.
- manage and test the auth process for an administrator.

This contract is also well documented even though it's an unusual situation (a protocol has some kind of public offices that hold a certain value and the owners have to pay periodically to keep the office). Anyways, while reading this article it's a good idea to also refer to the comments (or docs) in the shown code fragments.

# Writing the Contract

## Setup
Before starting to write the contract, you'll have to set up a Rust crate and add some configs to the `Cargo.toml` file:

```bash
cargo new --lib soroban-paulette-smart-contract
```

You also have to change your `Cargo.toml` file and have it look like the following (where we are simply specifying some things about our library and adding the soroban sdk with its auth helpers to the crate):

```toml
[package]
name = "soroban-paulette-smart-contract"
version = "0.0.0"
edition = "2021"
publish = false

[lib]
crate-type = ["cdylib", "rlib"]

[features]
testutils = ["soroban-sdk/testutils", "soroban-auth/testutils"]

[dependencies]
soroban-sdk = "0.1.0"
soroban-auth = "0.1.0"

[dev_dependencies]
soroban-sdk = { version = "0.1.0", features = ["testutils"] }
soroban-auth = { version = "0.1.0", features = ["testutils"] }
rand = { version = "0.7.3" }
```

You can also go ahead and create the `test.rs` and `testutils.rs` files for when we'll have to write our unit tests.

You're now all set and can start building the contract!

## Importing Contracts
As previously hinted we are going to import two contracts: the standard token contract and the dutch auction contract. To do so, you'll first have to build (or download) the two WASM binaries of the contracts (which you can find in the repo), and then use the `contractimport!` macro:

```rust
mod token {
    soroban_sdk::contractimport!(file = "./soroban_token_spec.wasm");
}

mod auction {
    use crate::{Identifier, Signature};
    soroban_sdk::contractimport!(file = "./soroban_dutch_auction_contract.wasm");
}
```

Over the next paragraphs, I'll dive into calling these two contracts from our "paulette" contract.

## Data Keys and Custom Types
Contract data works pretty much like a key-value store in Soroban, which means that things can get messy if we don't enforce a strong naming system for our keys. That is why we will define the `DataKey` enum:

```rust
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
```

### About custom types
On a production-level contract, it might be better to achieve further clarity in our code by creating new types or variants for values that can become ambiguous for not having their own descriptive type. For example, I have created a `TimeStamp` type for timestamps rather than using a `u64` type directly:

```rust
#[derive(Clone, PartialEq, PartialOrd, Eq, Ord, Debug)]
#[contracttype]
/// Timestamp type to enforce explicitness
pub struct TimeStamp(pub u64);
```

That obviously means that I also have to make some more implementations, for example one to sum two timestamps:

```rust
// Perform arithmetic ops on custom types
trait Arithmetic<Rhs = Self> {
    type Output;

    fn add(self, rhs: Rhs) -> Self::Output;
}

impl Arithmetic<TimeStamp> for TimeStamp {
    type Output = TimeStamp;

    fn add(self, other: Self) -> Self {
        Self(self.0 + other.0)
    }
}

```

I could have also used an alias (`type TimeStamp = u64`), but I wanted to leave room for further work on the `TimeStamp` type (invariants, covariants, impls, etc), as a matter of fact I have already added another implementation for this type to get the latest ledger timestamp:

```rust
impl TimeStamp {
    fn current(e: &Env) -> Self {
        Self(e.ledger().timestamp())
    }
}
```

Notice that I am using `environment.ledger().timestamp()` to access the ledger's current timestamp. Remember that in order to make smart contracts efficient and scalable, there is not much data that contracts are allowed to fetch. Still, things like the ledger's timestamp, protocol version and network passphrase can be accessed through `environment.ledger()`.

I have also added an `Auth` type to wrap better the admin auth (signature and nonce), and an office struct which will be the one stored in the contract data for each contract the admin creates and revokes:

```rust
#[derive(Clone)]
#[contracttype]
/// Auth type to wrap admin signature and nonce together
pub struct Auth {
    pub sig: Signature,
    pub nonce: BigInt,
}

#[derive(Clone)]
#[contracttype]
/// Office struct, stored with key DataKey::Bought(id)
pub struct Office {
    pub user: Identifier,
    pub expires: TimeStamp,
}
```

## Data helpers: writing and reading contract data
To interact with contract data in soroban, we use `environment.data()`:
- `environment.data().get(key)`: gets the value stored on the contract's data which is associated with the key `key`.
- `environment.data().set(key, value)`: creates or edits the value stored on the contract's data which is associated with the key `key` by setting `value` as value.

It is helpful to have functions perform the action of interacting with specific keys of the contract data:

```rust

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

```


## Interacting with the dutch auction contract
If you haven't already I recommend checking the [soroban-dutch-auction-contract README](https://github.com/Xycloo/soroban-dutch-auction-contract) to better understand what happens in the auctions.
To create a new auction, we have to initialize an existing auction contract (which needs to be registered to the environment). This means that rather than deploying the auction contract directly from the paulette contract through `env.deployer()`, we will leave the deployment of the auction contract to another contract (or manually), and only require the id to initialize the contract (i.e creating the auction).

Before initializing the contract we need a client to interact with it, we create one with:

```rust
let client = auction::Client::new(e, id); // id is the provided contract id
```

### Initializing the auction
We can then begin the auction by calling the contract's client just like the [auction contract's tests](https://github.com/Xycloo/soroban-dutch-auction-contract/blob/main/src/test.rs#L59):

```rust
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
```

### Making a bid
To place a bid we just create a new client (again) and call the buy function:

```rust
fn bid_auction(e: &Env, id: BytesN<32>, buyer: Identifier) -> bool {
    let client = auction::Client::new(e, id);
    client.buy(&buyer)
}
```

## Interacting with the token contract
Since we are going to need to transfer value from the buyers to the admin, we will need to use the standard token contract. More specifically, we will use the `token_client::xfer_from()` method which allows the contract to transfer a certain amount of the token from the buyer to a certain account, assuming that the buyer has previously allowed this transaction (using an allowance).

So, remembering that the `xfer_from` method looks like this:

```rust
fn xfer_from(e: Env, spender: Signature, nonce: BigInt, from: Identifier, to: Identifier, amount: BigInt);
```

We can write the following function, where `&read_administrator(e)` simply reads the ID of the admin. We'll dive further into working with the admin in the next section.

```rust
fn transfer_to_admin(e: &Env, from: Identifier, amount: BigInt) {
    let client = token::Client::new(e, get_token_id(e));

    client.xfer_from(
        &Signature::Invoker,
        &BigInt::zero(e),
        &from,
        &read_administrator(e), // reads the administrator's id
        &amount,
    )
}
```

## Admin management
In our contract, the admin acts pretty much like the King did (see [the historical inspiration](#inspiration)), meaning that they can create and revoke offices, and receive all the fees as well.

Below are the functions that the code uses when interacting with the admin:

```rust
/// checks if the contract already has an admin
fn has_administrator(e: &Env) -> bool {
    let key = DataKey::Admin;
    e.data().has(key)
}

/// return the contract's admin
fn read_administrator(e: &Env) -> Identifier {
    let key = DataKey::Admin;
    e.data().get_unchecked(key).unwrap()
}

/// set the admin
fn write_administrator(e: &Env, id: Identifier) {
    let key = DataKey::Admin;
    e.data().set(key, id);
}

/// assert that a signature is the admin's, used to authenticate the admin
fn check_admin(e: &Env, auth: &Signature) {
    let auth_id = auth.identifier(e);
    if auth_id != read_administrator(e) {
        panic!("not authorized by admin")
    }
}

```

We also need some kind of cryptographic commitment proving that the signature we are using hasn't been used already. We do that through the so-called `nonce`:

```rust
fn read_nonce(e: &Env, id: &Identifier) -> BigInt {
    let key = DataKey::Nonce(id.clone()); // remember the DataKey::Nonce
    e.data()
        .get(key)
        .unwrap_or_else(|| Ok(BigInt::zero(e)))
        .unwrap()
}

fn verify_and_consume_nonce(e: &Env, auth: &Signature, expected_nonce: &BigInt) {
    match auth {
        Signature::Invoker => { // when signature is directly from the invoker there is no need for the nonce 
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
    e.data().set(key, &nonce + 1); // increment the nonce
}

```

## Defining our contract trait
Rust traits are a great way to describe functions (as a matter of fact a trait) that an implementation has to satisfy. We can easily use a trait to summarize the implementations that our `PauletteContract` should implement:

```rust
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

```

Now that you have an overview of what the contract actually does, we can start writing these:

```rust
#[contractimpl]
impl PauletteContractTrait for PauletteContract {
   ...
}
```

## Initialization
This step is really just about one thing: setting in the contract's data the params that the contract needs to start working:
- the admin
- what token to use as currency (usdc for instance)
- how much taxes users have to pay in terms of the contract's currency (taxes in usdc for instance).

```rust
    fn initialize(e: Env, admin: Identifier, token_id: BytesN<32>, tax: BigInt) {
        if has_administrator(&e) {
            panic!("admin is already set");
        }

        write_administrator(&e, admin);
        put_token_id(&e, token_id);
        put_tax(&e, tax);
    }
```

Remember that all these functions we are using we already have defined, take a glance at the beginning of the article to see what they do.

## Creating a new office
This is slightly more complex than the initialization, in fact we have to:
- check that it's the admin providing authorization for a new office to be created
- since existing offices are stored with a 16 bytes array as ID, we need to check that the provided id doesn't already exist.
- create the office:
  - create a new auction
  - put the office in the contract's data with `DataKey::ForSale(16_bytes_id)` as key and the auction's id as value (so that we can interact with the office's auction without needing to know the ID of the auction (which is the id of the auction contract, the `auction` parameter in our case)):
  
```rust
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
```

where `make_new_office()` looks like this:

```rust
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
```
 Remember that [we have already talked about creating and bidding auctions](#interacting-with-the-dutch-auction-contract), and that put for sale is a simple function (which we have already seen in the beginning of the article) that writes the contract data.

## Buying an office that is for sale

Buying is also quite complex compared to the `initialize` method, we need to:
- get the specified office's auction id (which, if you remember, we had stored as the value for the office's id).
- make a bid to the auction (i.e buying the office).
- removing the `DataKey::ForSale(id)` data entry since we are going to add a `DataKey::Bought` entry through the `put_bought()` function, which has an `Office` struct as value where `office.user` is the buyer and `office.expires` (the expiration date) is the current ledger timestamp + 604800 (a week). "Why have such value hardcoded in the contract rather than setting it upon initialization?", you may be asking; there is no particular reason for this choice, I hardcoded it to show that it is also a viable option (and sometimes mandatory). Of course you could add another parameter in the `initialize` method, and write a put and get function for the new enum variant `DataKey::Interval`.

```rust
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
                expires: TimeStamp::current(&e).add(TimeStamp(604800)),
            },
        )
    }

```

## Paying the office's tax

Paying the taxes here is quite simple compared to the other methods:
- use the `transfer_to_admin` function to transfer the tax money to the admin.
- get `DataKey::Bought(id)` as a mutable and change its expiry to the previous expiry date + a week.

```rust
// the contract doesn't care if its the user who pays the office, just that someone is.
    fn pay_tax(e: Env, id: BytesN<16>, payer: Identifier) {
        transfer_to_admin(&e, payer, get_tax(&e));
        let mut office = get_bought(&e, id.clone());

        // dilemma: allow to pay taxes even after they have expired if the admin doesn't revoke the office?
        office.expires = office.expires.add(TimeStamp(604800));

        put_bought(&e, id, office);
    }
```

## Revoking the office

Once you understand the "creating an office" section, this one becomes really simple:
- check that it's the admin authorizing the action.
- assert that the office is really expired (`office.expires < TimeStamp::current(&e)`).
- remove the `DataKey::Bought(id)` entry to replace it with `DataKey::ForSale(id)` through the already-discussed `make_new_office()` function:

```rust
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

        if office.expires > TimeStamp::current(&e) {
            panic!("office is not expired yet");
        }

        remove_bought(&e, id.clone());
        make_new_office(&e, id, auction, price, min_price, slope);
    }
```

# Testing

We have now written our contract, and need proper testing to assert that it works as expected.

### Testutils
Our testutils will act as a skeleton contract that interacts with the actual contract but by easing some processes such as setting the environment source account when needed (admin invocations):

```rust
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
}

```

### Running tests

You can check out the tests in the `test.rs` file. The tests are intuitive but I'd like to shine light on a couple of things:
- we are generating the auction contract externally (from the contract) as discussed in the introduction.
- we are setting the ledger time with:
```rust
 e.ledger().set(LedgerInfo {
        timestamp: TIMESTAMP,
        protocol_version: 1,
        sequence_number: 10,
        network_passphrase: Default::default(),
        base_reserve: 10,
    });
```
This is needed because the price of the offices changes over time (dutch auction), and also because the offices expire over time.

- we are using the `approve` method from the token contract to allow the paulette contract to transfer the tokens out of the buyers (if you are confused by this behaviour, read [this section again](#interacting-with-the-token-contract)):
```rus
usdc_token.with_source_account(&user2).approve(
        &Signature::Invoker,
        &BigInt::zero(&e),
        &auction_contract_id,
        &paulette.get_price(office_id.clone()),
    );
```

If you now run the tests, you should see them all pass:

```bash
> cargo test
    Finished test [unoptimized + debuginfo] target(s) in 0.03s
     Running unittests src/lib.rs (target/debug/deps/soroban_paulette_smart_contract-43806aa4f75ee959)

running 3 tests
test test::test_invalid_admin - should panic ... ok
test test::test_invalid_revoke - should panic ... ok
test test::test_sequence ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s

   Doc-tests soroban-paulette-smart-contract

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

```

Thanks for reading and feel free to open issues in the repo if something is not clear or seems wrong.
