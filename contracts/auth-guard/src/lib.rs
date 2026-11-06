//! # auth-guard
//!
//! SorobanLens fixture contract for **authorization failures**.
//!
//! This contract holds an admin address and a single `u32` configuration
//! value. It deliberately exposes the two distinct ways authorization can fail
//! on Soroban, because a diagnostic tool must tell them apart:
//!
//! 1. **Host-level authorization failure**: [`AuthGuard::set_value`] calls
//!    `admin.require_auth()`. If the transaction has no valid authorization
//!    entry for the admin, the *Soroban host* aborts the invocation with
//!    `Error(Auth, InvalidAction)`. The contract never gets a chance to return
//!    its own error.
//! 2. **Contract-level authorization failure**: [`AuthGuard::set_value_as`]
//!    accepts any authenticated caller, then checks whether that caller is the
//!    admin. A properly signed call from a non-admin returns the contract error
//!    [`AuthGuardError::NotAdmin`] (`Error(Contract, #1)`).
//!
//! [`AuthGuard::transfer_admin`] requires authorization from *two* addresses
//! (current admin and new admin), demonstrating a partially-authorized
//! transaction.
//!
//! See `docs/contracts/auth-guard.md` for reproduction steps.

#![no_std]

use soroban_sdk::{contract, contracterror, contractevent, contractimpl, contracttype, Address, Env};

/// Ledgers in one day, assuming ~5 second close times.
const DAY_IN_LEDGERS: u32 = 17_280;
/// Extend instance storage TTL to ~30 days whenever it drops below ~29 days.
const INSTANCE_BUMP_AMOUNT: u32 = 30 * DAY_IN_LEDGERS;
const INSTANCE_LIFETIME_THRESHOLD: u32 = INSTANCE_BUMP_AMOUNT - DAY_IN_LEDGERS;

/// Contract-defined errors. Surfaced on-chain as `Error(Contract, #<code>)`.
///
/// Codes are part of the SorobanLens fixture contract: **never renumber them**.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum AuthGuardError {
    /// The caller authenticated successfully but is not the configured admin.
    NotAdmin = 1,
    /// `transfer_admin` was called with the current admin as the new admin.
    SameAdmin = 2,
}

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Admin,
    Value,
}

/// Emitted whenever the configuration value changes.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValueSet {
    #[topic]
    pub by: Address,
    pub value: u32,
}

/// Emitted when admin rights move to a new address.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminTransferred {
    #[topic]
    pub previous: Address,
    pub new_admin: Address,
}

#[contract]
pub struct AuthGuard;

#[contractimpl]
impl AuthGuard {
    /// Initializes the contract at deploy time. Constructors run exactly once,
    /// so there is no separate (and race-prone) `initialize` function.
    pub fn __constructor(env: Env, admin: Address, initial_value: u32) {
        let storage = env.storage().instance();
        storage.set(&DataKey::Admin, &admin);
        storage.set(&DataKey::Value, &initial_value);
    }

    /// Returns the current admin address.
    pub fn admin(env: Env) -> Address {
        read_admin(&env)
    }

    /// Returns the current configuration value.
    pub fn value(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::Value).unwrap_or(0)
    }

    /// Sets the configuration value. **Requires the admin's authorization.**
    ///
    /// Failure scenario: invoking without the admin's signature/authorization
    /// entry fails in the host with `Error(Auth, InvalidAction)`.
    pub fn set_value(env: Env, value: u32) {
        let admin = read_admin(&env);
        admin.require_auth();
        write_value(&env, &admin, value);
    }

    /// Sets the configuration value on behalf of `caller`.
    ///
    /// `caller` must authorize the call (host-level check). The contract then
    /// checks that `caller` is the admin (contract-level check), returning
    /// [`AuthGuardError::NotAdmin`] otherwise.
    pub fn set_value_as(env: Env, caller: Address, value: u32) -> Result<(), AuthGuardError> {
        caller.require_auth();
        if caller != read_admin(&env) {
            return Err(AuthGuardError::NotAdmin);
        }
        write_value(&env, &caller, value);
        Ok(())
    }

    /// Transfers admin rights. **Requires authorization from both the current
    /// admin and `new_admin`**, so a new admin cannot be assigned without
    /// consent (and an attacker cannot take over by naming themselves).
    pub fn transfer_admin(env: Env, new_admin: Address) -> Result<(), AuthGuardError> {
        let previous = read_admin(&env);
        if previous == new_admin {
            return Err(AuthGuardError::SameAdmin);
        }
        previous.require_auth();
        new_admin.require_auth();

        let storage = env.storage().instance();
        storage.set(&DataKey::Admin, &new_admin);
        storage.extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);

        AdminTransferred { previous, new_admin }.publish(&env);
        Ok(())
    }
}

fn read_admin(env: &Env) -> Address {
    // The constructor always sets the admin, so absence is an invariant
    // violation rather than a user error.
    env.storage()
        .instance()
        .get(&DataKey::Admin)
        .expect("admin is set by the constructor")
}

fn write_value(env: &Env, by: &Address, value: u32) {
    let storage = env.storage().instance();
    storage.set(&DataKey::Value, &value);
    storage.extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
    ValueSet {
        by: by.clone(),
        value,
    }
    .publish(env);
}

#[cfg(test)]
mod test;
