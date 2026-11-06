extern crate std;

use super::*;
use soroban_sdk::{
    testutils::Address as _,
    xdr::{ScErrorCode, ScErrorType},
    Address, Env, Error, Symbol,
};

fn setup() -> (Env, Address, ExecutionFailureClient<'static>) {
    let env = Env::default();
    let admin = Address::generate(&env);
    let id = env.register(ExecutionFailure, (&admin,));
    let client = ExecutionFailureClient::new(&env, &id);
    (env, admin, client)
}

/// The error that the SDK's `try_*` path reports for any non-contract host
/// error (granular codes are checked by the scenario-runner).
fn narrowed() -> Error {
    Error::from_type_and_code(ScErrorType::Context, ScErrorCode::InvalidAction)
}

#[test]
fn checked_add() {
    let (_env, _a, c) = setup();
    assert_eq!(c.checked_add(&2, &3), 5);
    assert_eq!(
        c.try_checked_add(&u64::MAX, &1),
        Err(Ok(ExecutionFailureError::Overflow))
    );
}

#[test]
fn unchecked_add_succeeds_without_overflow() {
    let (_env, _a, c) = setup();
    assert_eq!(c.unchecked_add(&2, &3), 5);
}

#[test]
fn unchecked_add_overflow_is_an_uncoded_failure() {
    let (_env, _a, c) = setup();
    // A bare panic: no contract error code is available to the caller.
    assert_eq!(c.try_unchecked_add(&u64::MAX, &1), Err(Ok(narrowed())));
}

#[test]
fn divide() {
    let (_env, _a, c) = setup();
    assert_eq!(c.divide(&10, &3), 3);
    assert_eq!(c.divide(&-10, &3), -3);
    assert_eq!(
        c.try_divide(&10, &0),
        Err(Ok(ExecutionFailureError::DivisionByZero))
    );
    assert_eq!(
        c.try_divide(&i128::MIN, &-1),
        Err(Ok(ExecutionFailureError::Overflow))
    );
}

#[test]
fn halt_preserves_contract_error_code() {
    let (_env, _a, c) = setup();
    // `halt` returns `()` rather than `Result`, so the client's error type is
    // the raw `soroban_sdk::Error`, carrying contract code #4.
    assert_eq!(
        c.try_halt(),
        Err(Ok(Error::from_contract_error(
            ExecutionFailureError::Halted as u32
        )))
    );
}

#[test]
fn entries_round_trip_and_missing_entry_is_reported() {
    let (env, _a, c) = setup();
    let key = Symbol::new(&env, "limit");
    assert_eq!(
        c.try_get_entry(&key),
        Err(Ok(ExecutionFailureError::MissingEntry))
    );
    env.mock_all_auths();
    c.set_entry(&key, &10);
    assert_eq!(c.get_entry(&key), 10);
}

#[test]
fn set_entry_requires_admin_auth() {
    let (env, _a, c) = setup();
    let key = Symbol::new(&env, "limit");
    assert_eq!(c.try_set_entry(&key, &10), Err(Ok(narrowed())));
    assert_eq!(
        c.try_get_entry(&key),
        Err(Ok(ExecutionFailureError::MissingEntry))
    );
}

#[test]
fn call_external_propagates_callee_contract_error() {
    let (env, _a, c) = setup();
    let other = env.register(ExecutionFailure, (Address::generate(&env),));
    // The callee fails with `Halted`; because `call_external` uses the
    // non-try `invoke_contract`, the whole invocation fails with the callee's
    // contract error code. (A successful cross-contract call is covered by
    // the `execution-failure/call_external/ok` fixture scenario.)
    assert_eq!(
        c.try_call_external(&other, &Symbol::new(&env, "halt")).err(),
        Some(Ok(Error::from_contract_error(
            ExecutionFailureError::Halted as u32
        )))
    );
}

#[test]
fn call_external_to_missing_contract_fails() {
    let (env, _a, c) = setup();
    let nowhere = Address::generate(&env);
    assert_eq!(
        c.try_call_external(&nowhere, &Symbol::new(&env, "value")).err(),
        Some(Ok(narrowed()))
    );
}

#[test]
fn call_external_to_missing_function_fails() {
    let (env, _a, c) = setup();
    let id = c.address.clone();
    assert_eq!(
        c.try_call_external(&id, &Symbol::new(&env, "does_not_exist"))
            .err(),
        Some(Ok(narrowed()))
    );
}

#[test]
fn hash_chain_is_deterministic() {
    let (env, _a, c) = setup();
    assert_eq!(c.hash_chain(&0), BytesN::from_array(&env, &[0u8; 32]));
    let one = c.hash_chain(&1);
    // sha256 of 32 zero bytes.
    let expected = env.crypto().sha256(&Bytes::from_array(&env, &[0u8; 32]));
    assert_eq!(one, BytesN::from(expected));
    assert_eq!(c.hash_chain(&5), c.hash_chain(&5));
}

#[test]
fn hash_chain_rejects_excessive_iterations() {
    let (_env, _a, c) = setup();
    assert_eq!(
        c.try_hash_chain(&(MAX_HASH_ITERATIONS + 1)),
        Err(Ok(ExecutionFailureError::TooManyIterations))
    );
}

#[test]
fn hash_chain_exhausts_default_budget() {
    let (env, _a, c) = setup();
    env.cost_estimate().budget().reset_default();
    // Budget exhaustion is not recoverable: the try_ client cannot return an
    // error value and the invocation aborts (a panic in the test env).
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        c.try_hash_chain(&MAX_HASH_ITERATIONS)
    }));
    assert!(r.is_err(), "expected the budget to be exceeded");
}

#[test]
fn hash_chain_small_input_fits_default_budget() {
    let (env, _a, c) = setup();
    env.cost_estimate().budget().reset_default();
    c.hash_chain(&100);
    assert!(env.cost_estimate().budget().cpu_instruction_cost() > 0);
}
