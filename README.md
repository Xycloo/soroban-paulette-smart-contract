# Soroban Offices Smart Contract

### Historical inspiration
The paulette, was a law enforced by France in the beginnings of the 17th century. It allowed the king to sell the ownership of public offices (making them hereditary). The owners of these public offices had to pay a tax each $\Delta t$ to keep a bought public office. These offices expire every $\Delta t$ and the the owner of each office has to pay a tax to maintain the ownership, if they didn't pay this tax, the office could be revoked by the king.

## Actual implementation Abstract
The contract admin can create new offices and put them for sale by invoking the [Soroban Dutch Auction Contract](https://github.com/Xycloo/soroban-dutch-auction-contract). This means that these offices start with a certain price which decreases with time. Users can then buy these offices at the auction's price and become the owners. The offices expire after a week ( $604800s$ ), and can be renewed by paying a tax (ideally less than the price at which the office was bought). If more than a week goes by and the owner hasn't paid the tax yet, the admin can revoke the office and put it for sale in an auction again. 


## What You'll Learn
This contract is quite complete in terms of used functionalities, by looking at the code and following this post you'll learn:
- how Soroban's contract data storage works for inserting, getting and deleting data.
- how to call a contract from within another contract by calling an external contract for the auction (and one for the standard token implementation).
- how to use your custom types for more explicitness in your code (in this contract only the `TimeStamp` type for simplicity, but on a production-level contract you may have to explicit most of the types that can become ambiguous to a future maintainer).
- using the standard token implementation to transfer tokens.
- manage and test the auth process for an administrator.

This contract is also well documented even though it's an edge case

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

[dependencies]0
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
As previously hinted we are going to import two contracts: the standard token contract and the dutch auction contract. To do so, you'll first have to build (or dowload) the two WASM binaries of the contracts (which you can find in the repo), and then use the `contractimport!` macro:

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
Contract data works pretty much like a key-value store in Soroban, that means that things can get messy if we don't enforce a strong naming system for our keys. That is why we will define the `DataKey` enum:

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
On a produciton-level contract, it might be better to achieve further clarity in our code by creating new types or variants for values that can become ambiguous for not having their own descriptive type. For example, I have created a `TimeStamp` type for timestamps rather than using a `u64` type directly:

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
- `environment.data().get(key)`: gets the value stored on the contract's data which is associated the the key `key`.
- `environment.data().set(key, value)`: creates or edits the value stored on the contract's data which is associated the the key `key` by setting `value` as value.

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
If you haven't already I recommed checking the [soroban-dutch-auction-contract README](https://github.com/Xycloo/soroban-dutch-auction-contract) to better understand what happens in the auctions.
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
Since we are going to need to transfer value from the buyers to the admin, we will need to use the standard token contract. More specifically, we will use the `token_client::xfer_from()` method which allows the contract to transfer a certain amount of the token from the buyer to a cecrtain account, assuming that the buyer has previously allowed this transaction (using an allowance).

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



## todos

- importing the token and auction contracts
- data keys, auth and timestamp types (+ its impls)
- data helpers (+ get_ts())
- new auction and bid auction fns
- xfer into contract vault
- manage admin + nonce
- contract trait
- contract initialization: writing data using data helpers
- new_office: verify admin, check bounds, mix new auction and put for sale inside the make_new_office fn. 
- buy office: get data, bidding, remove for sale and put bought with its owner (notice how the money goes into the auction's admin account, not necessarily the paulette contract vault)
- pay_tax: using the xfer directly (used indirectly in the auction contract), getting the office and changing its expiry
- revoke: check admin and bounds, remove bought office and make a new one (i.e new auction + put_for_sale)
- testutils
- testing
