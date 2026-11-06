//! # execution-failure
//!
//! SorobanLens fixture contract for **runtime execution failures**.
//!
//! Each function demonstrates one predictable failure mode so diagnostic rules
//! can be validated against known ground truth. Several functions come in a
//! *graceful* and an *ungraceful* variant, because they surface very
//! differently on-chain:
//!
//! | Function            | Failure mode                          | Surfaced as (diagnostic events)   |
//! |---------------------|---------------------------------------|-----------------------------------|
//! | `checked_add`       | overflow handled by contract          | `Error(Contract, #2)`             |
//! | `unchecked_add`     | overflow panic (`overflow-checks`)    | `Error(WasmVm, InvalidAction)`    |
//! | `divide`            | division by zero handled by contract  | `Error(Contract, #1)`             |
//! | `halt`              | `panic_with_error!`                   | `Error(Contract, #4)`             |
//! | `get_entry`         | missing storage entry, handled        | `Error(Contract, #3)`             |
//! | `call_external`     | cross-contract call to missing contract | `Error(Storage, MissingValue)`  |
//! | `hash_chain`        | CPU budget exhaustion                 | `Error(Budget, ExceededLimit)`    |
//!
//! The exact codes above are asserted by this crate's tests and by the
//! `scenario-runner` crate against `fixtures/scenarios.json`.

#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, Address, Bytes, BytesN, Env,
    Symbol, Val, Vec,
};

const DAY_IN_LEDGERS: u32 = 17_280;
const BUMP_AMOUNT: u32 = 30 * DAY_IN_LEDGERS;
const LIFETIME_THRESHOLD: u32 = BUMP_AMOUNT - DAY_IN_LEDGERS;

/// Upper bound on `hash_chain` iterations. Large enough to exceed the network
/// CPU budget, small enough that the loop itself is bounded.
pub const MAX_HASH_ITERATIONS: u32 = 1_000_000;

/// Contract-defined errors. Surfaced on-chain as `Error(Contract, #<code>)`.
///
/// Codes are part of the SorobanLens fixture contract: **never renumber them**.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ExecutionFailureError {
    /// `divide` was called with a zero divisor.
    DivisionByZero = 1,
    /// `checked_add` would overflow `u64`.
    Overflow = 2,
    /// `get_entry` found no value stored under the requested key.
    MissingEntry = 3,
    /// `halt` aborted execution deliberately via `panic_with_error!`.
    Halted = 4,
    /// `hash_chain` was asked for more than [`MAX_HASH_ITERATIONS`].
    TooManyIterations = 5,
}

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Admin,
    Entry(Symbol),
}

#[contract]
pub struct ExecutionFailure;

#[contractimpl]
impl ExecutionFailure {
    pub fn __constructor(env: Env, admin: Address) {
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    /// Adds two numbers, returning [`ExecutionFailureError::Overflow`] instead
    /// of panicking. This is the recommended pattern.
    pub fn checked_add(_env: Env, a: u64, b: u64) -> Result<u64, ExecutionFailureError> {
        a.checked_add(b).ok_or(ExecutionFailureError::Overflow)
    }

    /// Adds two numbers with plain `+`. With `overflow-checks = true` (the
    /// recommended release profile) an overflow **panics**, which the host
    /// reports as a generic `Error(WasmVm, InvalidAction)` with no
    /// contract-specific code. This shows why unstructured panics are hard to
    /// diagnose.
    #[allow(clippy::arithmetic_side_effects)]
    pub fn unchecked_add(_env: Env, a: u64, b: u64) -> u64 {
        a + b
    }

    /// Integer division returning [`ExecutionFailureError::DivisionByZero`]
    /// for a zero divisor. `i128::MIN / -1` overflows and is reported as
    /// [`ExecutionFailureError::Overflow`].
    pub fn divide(_env: Env, numerator: i128, denominator: i128) -> Result<i128, ExecutionFailureError> {
        if denominator == 0 {
            return Err(ExecutionFailureError::DivisionByZero);
        }
        numerator
            .checked_div(denominator)
            .ok_or(ExecutionFailureError::Overflow)
    }

    /// Always aborts with `panic_with_error!`, which, unlike a bare panic,
    /// preserves a contract error code (`Error(Contract, #4)`).
    pub fn halt(env: Env) {
        panic_with_error!(&env, ExecutionFailureError::Halted);
    }

    /// Stores `value` under `key`. Requires the admin's authorization.
    pub fn set_entry(env: Env, key: Symbol, value: u32) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("admin is set by the constructor");
        admin.require_auth();

        let k = DataKey::Entry(key);
        let persistent = env.storage().persistent();
        persistent.set(&k, &value);
        persistent.extend_ttl(&k, LIFETIME_THRESHOLD, BUMP_AMOUNT);
    }

    /// Reads the value stored under `key`, returning
    /// [`ExecutionFailureError::MissingEntry`] when absent.
    pub fn get_entry(env: Env, key: Symbol) -> Result<u32, ExecutionFailureError> {
        env.storage()
            .persistent()
            .get(&DataKey::Entry(key))
            .ok_or(ExecutionFailureError::MissingEntry)
    }

    /// Invokes `func` (with no arguments) on another contract and returns its
    /// raw result.
    ///
    /// Pointing `contract` at an address with no deployed contract makes the
    /// host fail the cross-contract call. This is the "missing contract
    /// reference" scenario.
    pub fn call_external(env: Env, contract: Address, func: Symbol) -> Val {
        env.invoke_contract::<Val>(&contract, &func, Vec::new(&env))
    }

    /// Applies SHA-256 `iterations` times to a 32-byte seed and returns the
    /// final digest.
    ///
    /// Each iteration costs a host call and CPU instructions, so a large
    /// `iterations` exhausts the transaction's CPU budget, which is reported as
    /// `Error(Budget, ExceededLimit)`. Budget errors are **not recoverable**:
    /// they cannot be caught by a calling contract.
    pub fn hash_chain(env: Env, iterations: u32) -> Result<BytesN<32>, ExecutionFailureError> {
        if iterations > MAX_HASH_ITERATIONS {
            return Err(ExecutionFailureError::TooManyIterations);
        }
        let mut digest: BytesN<32> = BytesN::from_array(&env, &[0u8; 32]);
        for _ in 0..iterations {
            let input: Bytes = digest.into();
            digest = env.crypto().sha256(&input).into();
        }
        Ok(digest)
    }
}

#[cfg(test)]
mod test;
