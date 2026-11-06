extern crate std;

use super::*;
use soroban_sdk::{
    testutils::{
        Address as _, AuthorizedFunction, AuthorizedInvocation, Events as _, MockAuth, MockAuthInvoke,
    },
    Address, Env, Error, Event as _, IntoVal, Symbol,
};

fn setup() -> (Env, Address, AuthGuardClient<'static>) {
    let env = Env::default();
    let admin = Address::generate(&env);
    let id = env.register(AuthGuard, (&admin, 1u32));
    let client = AuthGuardClient::new(&env, &id);
    (env, admin, client)
}

/// Authorizes exactly one root invocation for one address.
fn sign(
    env: &Env,
    client: &AuthGuardClient,
    signer: &Address,
    fn_name: &str,
    args: soroban_sdk::Vec<soroban_sdk::Val>,
) {
    env.mock_auths(&[MockAuth {
        address: signer,
        invoke: &MockAuthInvoke {
            contract: &client.address,
            fn_name,
            args,
            sub_invokes: &[],
        },
    }]);
}

#[test]
fn constructor_sets_admin_and_value() {
    let (_env, admin, client) = setup();
    assert_eq!(client.admin(), admin);
    assert_eq!(client.value(), 1);
}

#[test]
fn set_value_requires_exactly_the_admins_auth() {
    let (env, admin, client) = setup();
    env.mock_all_auths();
    client.set_value(&42);

    // `env.auths()` reflects only the most recent invocation, so check it
    // before calling any other contract function.
    assert_eq!(
        env.auths(),
        std::vec![(
            admin.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    client.address.clone(),
                    Symbol::new(&env, "set_value"),
                    (42u32,).into_val(&env),
                )),
                sub_invocations: std::vec![],
            }
        )]
    );
    assert_eq!(client.value(), 42);
}

#[test]
fn set_value_emits_event() {
    let (env, admin, client) = setup();
    env.mock_all_auths();
    client.set_value(&9);
    assert_eq!(
        env.events().all(),
        std::vec![ValueSet { by: admin, value: 9 }.to_xdr(&env, &client.address)]
    );
}

#[test]
fn set_value_without_auth_fails_in_host() {
    let (_env, _admin, client) = setup();
    // Through the SDK's try_ path, host errors are narrowed to
    // Error(Context, InvalidAction); the granular Error(Auth, InvalidAction)
    // is verified by the scenario-runner, which calls the host directly.
    let err = client.try_set_value(&42).unwrap_err();
    assert_eq!(
        err,
        Ok(Error::from_type_and_code(
            soroban_sdk::xdr::ScErrorType::Context,
            soroban_sdk::xdr::ScErrorCode::InvalidAction,
        ))
    );
    assert_eq!(client.value(), 1, "state must be unchanged");
}

#[test]
fn set_value_signed_by_wrong_account_fails() {
    let (env, _admin, client) = setup();
    let mallory = Address::generate(&env);
    sign(&env, &client, &mallory, "set_value", (42u32,).into_val(&env));
    assert!(client.try_set_value(&42).is_err());
    assert_eq!(client.value(), 1);
}

#[test]
fn set_value_as_rejects_authenticated_non_admin() {
    let (env, _admin, client) = setup();
    let mallory = Address::generate(&env);
    sign(
        &env,
        &client,
        &mallory,
        "set_value_as",
        (&mallory, 7u32).into_val(&env),
    );
    assert_eq!(
        client.try_set_value_as(&mallory, &7),
        Err(Ok(AuthGuardError::NotAdmin))
    );
    assert_eq!(client.value(), 1);
}

#[test]
fn set_value_as_accepts_admin() {
    let (env, admin, client) = setup();
    sign(
        &env,
        &client,
        &admin,
        "set_value_as",
        (&admin, 7u32).into_val(&env),
    );
    client.set_value_as(&admin, &7);
    assert_eq!(client.value(), 7);
}

#[test]
fn transfer_admin_requires_both_parties() {
    let (env, admin, client) = setup();
    let alice = Address::generate(&env);
    let args: soroban_sdk::Vec<soroban_sdk::Val> = (&alice,).into_val(&env);

    // Only the current admin signs: fails, admin unchanged.
    sign(&env, &client, &admin, "transfer_admin", args.clone());
    assert!(client.try_transfer_admin(&alice).is_err());
    assert_eq!(client.admin(), admin);

    // Both sign: succeeds.
    let invoke = MockAuthInvoke {
        contract: &client.address,
        fn_name: "transfer_admin",
        args,
        sub_invokes: &[],
    };
    env.mock_auths(&[
        MockAuth {
            address: &admin,
            invoke: &invoke,
        },
        MockAuth {
            address: &alice,
            invoke: &invoke,
        },
    ]);
    client.transfer_admin(&alice);
    // Events, like auths, reflect only the most recent invocation.
    assert_eq!(
        env.events().all(),
        std::vec![AdminTransferred {
            previous: admin,
            new_admin: alice.clone()
        }
        .to_xdr(&env, &client.address)]
    );
    assert_eq!(client.admin(), alice);
}

#[test]
fn transfer_admin_to_self_is_rejected() {
    let (env, admin, client) = setup();
    env.mock_all_auths();
    assert_eq!(
        client.try_transfer_admin(&admin),
        Err(Ok(AuthGuardError::SameAdmin))
    );
}

#[test]
fn old_admin_loses_rights_after_transfer() {
    let (env, admin, client) = setup();
    let alice = Address::generate(&env);
    env.mock_all_auths();
    client.transfer_admin(&alice);
    assert_eq!(
        client.try_set_value_as(&admin, &5),
        Err(Ok(AuthGuardError::NotAdmin))
    );
    client.set_value_as(&alice, &5);
    assert_eq!(client.value(), 5);
}
