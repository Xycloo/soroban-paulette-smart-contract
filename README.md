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
