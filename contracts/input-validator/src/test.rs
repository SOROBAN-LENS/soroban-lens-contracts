extern crate std;

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events as _, Ledger as _},
    Address, Env, Event as _, String,
};

const NOW: u32 = 1_000;

fn setup() -> (Env, Address, InputValidatorClient<'static>) {
    let env = Env::default();
    env.ledger().set_sequence_number(NOW);
    env.mock_all_auths();
    let id = env.register(InputValidator, ());
    let owner = Address::generate(&env);
    let client = InputValidatorClient::new(&env, &id);
    (env, owner, client)
}

fn memo(env: &Env, s: &str) -> String {
    String::from_str(env, s)
}

#[test]
fn creates_order_and_assigns_sequential_ids() {
    let (env, owner, c) = setup();
    assert_eq!(c.create_order(&owner, &100, &5, &(NOW + 10), &memo(&env, "a")), 0);
    assert_eq!(c.create_order(&owner, &200, &6, &(NOW + 10), &memo(&env, "b")), 1);
    assert_eq!(c.order_count(), 2);

    let order = c.get_order(&1);
    assert_eq!(
        order,
        Order {
            owner,
            amount: 200,
            price: 6,
            expiry_ledger: NOW + 10,
            memo: memo(&env, "b"),
        }
    );
}

#[test]
fn create_order_emits_event() {
    let (env, owner, c) = setup();
    c.create_order(&owner, &100, &5, &(NOW + 10), &memo(&env, ""));
    assert_eq!(
        env.events().all(),
        std::vec![OrderCreated {
            owner,
            id: 0,
            amount: 100,
            price: 5
        }
        .to_xdr(&env, &c.address)]
    );
}

#[test]
fn boundary_values_are_accepted() {
    let (env, owner, c) = setup();
    let max_memo = "x".repeat(MAX_MEMO_LEN as usize);
    c.validate_order(&1, &1, &(NOW + 1), &memo(&env, &max_memo));
    c.validate_order(
        &MAX_AMOUNT,
        &i128::MAX,
        &(NOW + MAX_EXPIRY_WINDOW),
        &memo(&env, ""),
    );
    c.create_order(
        &owner,
        &MAX_AMOUNT,
        &1,
        &(NOW + MAX_EXPIRY_WINDOW),
        &memo(&env, &max_memo),
    );
}

#[test]
fn each_rule_maps_to_its_error() {
    use InputValidatorError::*;
    let (env, owner, c) = setup();
    let ok_memo = memo(&env, "ok");
    let cases: [(i128, i128, u32, String, InputValidatorError); 8] = [
        (0, 5, NOW + 1, ok_memo.clone(), AmountNotPositive),
        (-1, 5, NOW + 1, ok_memo.clone(), AmountNotPositive),
        (MAX_AMOUNT + 1, 5, NOW + 1, ok_memo.clone(), AmountTooLarge),
        (100, 0, NOW + 1, ok_memo.clone(), PriceNotPositive),
        (100, 5, NOW, ok_memo.clone(), ExpiryInPast),
        (100, 5, NOW - 1, ok_memo.clone(), ExpiryInPast),
        (100, 5, NOW + MAX_EXPIRY_WINDOW + 1, ok_memo.clone(), ExpiryTooFar),
        (
            100,
            5,
            NOW + 1,
            memo(&env, &"x".repeat(MAX_MEMO_LEN as usize + 1)),
            MemoTooLong,
        ),
    ];
    for (amount, price, expiry, m, expected) in cases {
        assert_eq!(
            c.try_validate_order(&amount, &price, &expiry, &m),
            Err(Ok(expected))
        );
        assert_eq!(
            c.try_create_order(&owner, &amount, &price, &expiry, &m),
            Err(Ok(expected))
        );
    }
    assert_eq!(c.order_count(), 0, "failed creations must not consume ids");
}

#[test]
fn rules_are_checked_in_documented_order() {
    let (env, _owner, c) = setup();
    // Every argument is invalid; the amount rule is first.
    let bad_memo = memo(&env, &"x".repeat(64));
    assert_eq!(
        c.try_validate_order(&0, &0, &0, &bad_memo),
        Err(Ok(InputValidatorError::AmountNotPositive))
    );
}

#[test]
fn memo_limit_is_in_bytes_not_characters() {
    let (env, _owner, c) = setup();
    // 11 three-byte characters = 33 bytes > 32, though only 11 characters.
    let m = memo(&env, &"€".repeat(11));
    assert_eq!(
        c.try_validate_order(&1, &1, &(NOW + 1), &m),
        Err(Ok(InputValidatorError::MemoTooLong))
    );
}

#[test]
fn get_missing_order_returns_not_found() {
    let (_env, _owner, c) = setup();
    assert_eq!(c.try_get_order(&0), Err(Ok(InputValidatorError::OrderNotFound)));
}

#[test]
fn create_order_requires_owner_auth() {
    let env = Env::default();
    env.ledger().set_sequence_number(NOW);
    let c = InputValidatorClient::new(&env, &env.register(InputValidator, ()));
    let owner = Address::generate(&env);
    assert!(c
        .try_create_order(&owner, &100, &5, &(NOW + 1), &memo(&env, ""))
        .is_err());
    assert_eq!(c.order_count(), 0);
}
