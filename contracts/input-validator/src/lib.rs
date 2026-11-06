//! # input-validator
//!
//! SorobanLens fixture contract for **invalid argument** failures.
//!
//! A minimal limit-order registry. Every user-supplied argument is validated
//! and each validation rule maps to exactly one stable contract error code, so
//! a diagnostic tool can translate `Error(Contract, #N)` into a precise,
//! human-readable explanation.
//!
//! Validation rules (see [`InputValidatorError`] for codes):
//!
//! | Argument        | Rule                                              |
//! |-----------------|---------------------------------------------------|
//! | `amount`        | `> 0` and `<= MAX_AMOUNT`                          |
//! | `price`         | `> 0`                                              |
//! | `expiry_ledger` | `> current ledger` and `<= current + MAX_EXPIRY_WINDOW` |
//! | `memo`          | at most `MAX_MEMO_LEN` bytes                       |
//!
//! Note: arguments of the *wrong type* (e.g. a string where an `i128` is
//! expected) never reach this code. The host rejects them before the contract
//! runs, which is a different failure class (see `docs/diagnostic-terminology.md`).

#![no_std]

use soroban_sdk::{contract, contracterror, contractevent, contractimpl, contracttype, Address, Env, String};

/// Largest accepted order amount (1 billion units at 7 decimals).
pub const MAX_AMOUNT: i128 = 1_000_000_000 * 10_000_000;
/// Longest accepted memo, in bytes.
pub const MAX_MEMO_LEN: u32 = 32;
/// Furthest in the future an order may expire, in ledgers (~31 days).
pub const MAX_EXPIRY_WINDOW: u32 = 31 * DAY_IN_LEDGERS;

const DAY_IN_LEDGERS: u32 = 17_280;
const INSTANCE_BUMP_AMOUNT: u32 = 30 * DAY_IN_LEDGERS;
const INSTANCE_LIFETIME_THRESHOLD: u32 = INSTANCE_BUMP_AMOUNT - DAY_IN_LEDGERS;
/// Orders live slightly longer than the maximum expiry window.
const ORDER_BUMP_AMOUNT: u32 = MAX_EXPIRY_WINDOW + DAY_IN_LEDGERS;
const ORDER_LIFETIME_THRESHOLD: u32 = ORDER_BUMP_AMOUNT - DAY_IN_LEDGERS;

/// Contract-defined errors. Surfaced on-chain as `Error(Contract, #<code>)`.
///
/// Codes are part of the SorobanLens fixture contract: **never renumber them**.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum InputValidatorError {
    /// `amount` was zero or negative.
    AmountNotPositive = 1,
    /// `amount` exceeded [`MAX_AMOUNT`].
    AmountTooLarge = 2,
    /// `price` was zero or negative.
    PriceNotPositive = 3,
    /// `expiry_ledger` was at or before the current ledger sequence.
    ExpiryInPast = 4,
    /// `expiry_ledger` was more than [`MAX_EXPIRY_WINDOW`] ledgers ahead.
    ExpiryTooFar = 5,
    /// `memo` was longer than [`MAX_MEMO_LEN`] bytes.
    MemoTooLong = 6,
    /// No order exists with the requested id.
    OrderNotFound = 7,
}

/// A stored limit order.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Order {
    pub owner: Address,
    pub amount: i128,
    pub price: i128,
    pub expiry_ledger: u32,
    pub memo: String,
}

#[contracttype]
#[derive(Clone)]
enum DataKey {
    /// Next order id (instance storage).
    NextId,
    /// An order by id (persistent storage).
    Order(u64),
}

/// Emitted when an order is created.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrderCreated {
    #[topic]
    pub owner: Address,
    pub id: u64,
    pub amount: i128,
    pub price: i128,
}

#[contract]
pub struct InputValidator;

#[contractimpl]
impl InputValidator {
    /// Creates an order owned by `owner` and returns its id.
    ///
    /// `owner` must authorize the call. All arguments are validated *after*
    /// authorization; the first failing rule determines the returned error.
    pub fn create_order(
        env: Env,
        owner: Address,
        amount: i128,
        price: i128,
        expiry_ledger: u32,
        memo: String,
    ) -> Result<u64, InputValidatorError> {
        owner.require_auth();
        validate(&env, amount, price, expiry_ledger, &memo)?;

        let instance = env.storage().instance();
        let id: u64 = instance.get(&DataKey::NextId).unwrap_or(0);
        // u64 cannot realistically overflow here, but never wrap silently.
        let next = id.checked_add(1).expect("order id space exhausted");
        instance.set(&DataKey::NextId, &next);
        instance.extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);

        let key = DataKey::Order(id);
        let order = Order {
            owner: owner.clone(),
            amount,
            price,
            expiry_ledger,
            memo,
        };
        let persistent = env.storage().persistent();
        persistent.set(&key, &order);
        persistent.extend_ttl(&key, ORDER_LIFETIME_THRESHOLD, ORDER_BUMP_AMOUNT);

        OrderCreated {
            owner,
            id,
            amount,
            price,
        }
        .publish(&env);
        Ok(id)
    }

    /// Validates order parameters without storing anything.
    ///
    /// Useful for simulating a call to check inputs before submitting.
    pub fn validate_order(
        env: Env,
        amount: i128,
        price: i128,
        expiry_ledger: u32,
        memo: String,
    ) -> Result<(), InputValidatorError> {
        validate(&env, amount, price, expiry_ledger, &memo)
    }

    /// Returns an order by id, or [`InputValidatorError::OrderNotFound`].
    pub fn get_order(env: Env, id: u64) -> Result<Order, InputValidatorError> {
        env.storage()
            .persistent()
            .get(&DataKey::Order(id))
            .ok_or(InputValidatorError::OrderNotFound)
    }

    /// Returns how many orders have been created.
    pub fn order_count(env: Env) -> u64 {
        env.storage().instance().get(&DataKey::NextId).unwrap_or(0)
    }
}

/// Applies every validation rule in a fixed, documented order.
fn validate(
    env: &Env,
    amount: i128,
    price: i128,
    expiry_ledger: u32,
    memo: &String,
) -> Result<(), InputValidatorError> {
    if amount <= 0 {
        return Err(InputValidatorError::AmountNotPositive);
    }
    if amount > MAX_AMOUNT {
        return Err(InputValidatorError::AmountTooLarge);
    }
    if price <= 0 {
        return Err(InputValidatorError::PriceNotPositive);
    }
    let current = env.ledger().sequence();
    if expiry_ledger <= current {
        return Err(InputValidatorError::ExpiryInPast);
    }
    // Saturating so the window can never wrap. (Not reachable in practice:
    // the host rejects ledger sequences this close to u32::MAX because entry
    // TTLs would overflow first, so there is no unit test for it.)
    if expiry_ledger > current.saturating_add(MAX_EXPIRY_WINDOW) {
        return Err(InputValidatorError::ExpiryTooFar);
    }
    if memo.len() > MAX_MEMO_LEN {
        return Err(InputValidatorError::MemoTooLong);
    }
    Ok(())
}

#[cfg(test)]
mod test;
