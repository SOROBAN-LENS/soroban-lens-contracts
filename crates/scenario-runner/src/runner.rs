//! Executes catalog scenarios in a fresh soroban-sdk test environment.
//!
//! Contracts are registered **natively** (compiled into this test binary), not
//! as Wasm. Native execution reproduces contract logic, storage, auth and
//! host-function budget accounting faithfully, but it does *not* meter Wasm
//! instructions, and a bare Rust panic is mapped to the error a Wasm trap would
//! produce. See `docs/verification.md` for what this does and does not prove.

use crate::catalog::{ArgValue, AuthMode, BudgetMode, Catalog, Expected, Scenario};
use soroban_env_host::{Env as _, HostError};
use soroban_sdk::{
    testutils::{Address as _, Ledger as _, MockAuth, MockAuthInvoke},
    Address, Env, Error, IntoVal, String as SorobanString, Symbol, TryFromVal, Val, Vec,
};
use std::collections::BTreeMap;
use std::fmt;

/// The result of running one scenario.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Observation {
    Success { return_value: Option<ArgValue> },
    Failure { error: String },
}

impl fmt::Display for Observation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Observation::Success {
                return_value: Some(v),
            } => write!(f, "success -> {v:?}"),
            Observation::Success { return_value: None } => write!(f, "success"),
            Observation::Failure { error } => write!(f, "failure: {error}"),
        }
    }
}

/// Why a scenario could not be executed (as opposed to a mismatch).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunError {
    UnknownContract(String),
    InvalidArg(String),
    SetupFailed { function: String, error: String },
}

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunError::UnknownContract(c) => write!(f, "unknown contract `{c}`"),
            RunError::InvalidArg(m) => write!(f, "invalid argument: {m}"),
            RunError::SetupFailed { function, error } => {
                write!(f, "setup call `{function}` failed: {error}")
            }
        }
    }
}

impl std::error::Error for RunError {}

/// Per-run world: the environment, named actors and deployed contracts.
struct World {
    env: Env,
    actors: BTreeMap<String, Address>,
    contracts: BTreeMap<String, Address>,
}

impl World {
    fn new(env: Env) -> Self {
        Self {
            env,
            actors: BTreeMap::new(),
            contracts: BTreeMap::new(),
        }
    }

    /// Resolves an actor name to an address, generating one on first use.
    fn address(&mut self, actor: &str) -> Result<Address, RunError> {
        if let Some(name) = actor.strip_prefix('@') {
            return self
                .contracts
                .get(name)
                .cloned()
                .ok_or_else(|| RunError::UnknownContract(name.to_string()));
        }
        let env = &self.env;
        Ok(self
            .actors
            .entry(actor.to_string())
            // `Address::generate` yields a fresh address with nothing deployed
            // at it, which is exactly what `unregistered_contract` needs too.
            .or_insert_with(|| Address::generate(env))
            .clone())
    }

    fn resolve_arg(&mut self, arg: &ArgValue) -> Result<Val, RunError> {
        let env = self.env.clone();
        let parse_err = |t: &str, v: &str| RunError::InvalidArg(format!("cannot parse `{v}` as {t}"));
        Ok(match arg {
            ArgValue::Void => ().into_val(&env),
            ArgValue::Bool { value } => value.into_val(&env),
            ArgValue::U32 { value } => value.into_val(&env),
            ArgValue::U64 { value } => value
                .parse::<u64>()
                .map_err(|_| parse_err("u64", value))?
                .into_val(&env),
            ArgValue::I128 { value } => value
                .parse::<i128>()
                .map_err(|_| parse_err("i128", value))?
                .into_val(&env),
            ArgValue::String { value } => SorobanString::from_str(&env, value).into_val(&env),
            ArgValue::Symbol { value } => Symbol::new(&env, value).into_val(&env),
            ArgValue::Address { actor } => self.address(actor)?.into_val(&env),
        })
    }

    fn resolve_args(&mut self, args: &[ArgValue]) -> Result<Vec<Val>, RunError> {
        let mut out = Vec::new(&self.env);
        for a in args {
            out.push_back(self.resolve_arg(a)?);
        }
        Ok(out)
    }

    /// Invokes a contract function through the host directly.
    ///
    /// The SDK's `try_invoke_contract` narrows every non-contract host error
    /// to `Error(Context, InvalidAction)` (by design, see
    /// `soroban-env-host/src/host.rs`, `try_call`) and panics on
    /// non-recoverable errors. Calling `Host::call` instead returns the full
    /// `HostError`, i.e. the same granular error that appears in on-chain
    /// diagnostic events.
    fn invoke(&self, contract: &Address, function: &str, args: Vec<Val>) -> Result<Val, Error> {
        let host = self.env.host();
        let func = Symbol::new(&self.env, function);
        host.call(contract.to_object(), func.to_symbol_val(), args.to_object())
            .map_err(|e: HostError| e.error)
    }
}

/// Converts a returned `Val` into the same JSON-typed shape as `expected`,
/// so the two can be compared. Returns `None` if the value does not have the
/// expected type.
fn decode_like(env: &Env, val: Val, expected: &ArgValue) -> Option<ArgValue> {
    Some(match expected {
        ArgValue::Void => {
            <()>::try_from_val(env, &val).ok()?;
            ArgValue::Void
        }
        ArgValue::Bool { .. } => ArgValue::Bool {
            value: bool::try_from_val(env, &val).ok()?,
        },
        ArgValue::U32 { .. } => ArgValue::U32 {
            value: u32::try_from_val(env, &val).ok()?,
        },
        ArgValue::U64 { .. } => ArgValue::U64 {
            value: u64::try_from_val(env, &val).ok()?.to_string(),
        },
        ArgValue::I128 { .. } => ArgValue::I128 {
            value: i128::try_from_val(env, &val).ok()?.to_string(),
        },
        ArgValue::String { .. } => ArgValue::String {
            value: SorobanString::try_from_val(env, &val).ok()?.to_string(),
        },
        ArgValue::Symbol { .. } => ArgValue::Symbol {
            value: Symbol::try_from_val(env, &val).ok()?.to_string(),
        },
        // Generated addresses differ per run; only the type is comparable.
        ArgValue::Address { actor } => {
            Address::try_from_val(env, &val).ok()?;
            ArgValue::Address { actor: actor.clone() }
        }
    })
}

/// Registers a contract by catalog name with its constructor arguments.
fn deploy(world: &mut World, catalog: &Catalog, name: &str) -> Result<Address, RunError> {
    let info = catalog
        .contracts
        .get(name)
        .ok_or_else(|| RunError::UnknownContract(name.to_string()))?;
    let args = world.resolve_args(&info.constructor_args)?;
    let env = &world.env;
    let id = match name {
        "auth-guard" => env.register(auth_guard::AuthGuard, args),
        "input-validator" => env.register(input_validator::InputValidator, args),
        "execution-failure" => env.register(execution_failure::ExecutionFailure, args),
        other => return Err(RunError::UnknownContract(other.to_string())),
    };
    world.contracts.insert(name.to_string(), id.clone());
    Ok(id)
}

/// A scenario environment ready for the measured invocation.
struct Prepared {
    world: World,
    target: Address,
    args: Vec<Val>,
}

/// Builds a fresh environment: deploys contracts, runs setup calls,
/// configures authorization and resets the budget. Everything up to, but not
/// including, the scenario invocation.
fn prepare(catalog: &Catalog, scenario: &Scenario) -> Result<Prepared, RunError> {
    let env = Env::default();
    if let Some(seq) = scenario.setup.ledger_sequence {
        env.ledger().set_sequence_number(seq);
    }
    let mut world = World::new(env.clone());

    // Deploy every contract so `@<name>` references always resolve.
    // Deployment (constructors) is not part of the measured invocation.
    env.cost_estimate().budget().reset_unlimited();
    let mut target = None;
    for name in catalog.contracts.keys() {
        let id = deploy(&mut world, catalog, name)?;
        if *name == scenario.contract {
            target = Some(id);
        }
    }
    let target = target.ok_or_else(|| RunError::UnknownContract(scenario.contract.clone()))?;

    if !scenario.setup.calls.is_empty() {
        env.mock_all_auths();
        for call in &scenario.setup.calls {
            let args = world.resolve_args(&call.args)?;
            world
                .invoke(&target, &call.function, args)
                .map_err(|e| RunError::SetupFailed {
                    function: call.function.clone(),
                    error: format!("{e:?}"),
                })?;
        }
    }

    let args = world.resolve_args(&scenario.args)?;

    match &scenario.auth {
        AuthMode::None => env.set_auths(&[]),
        AuthMode::MockAll => env.mock_all_auths(),
        AuthMode::Signers(signers) => {
            let addrs = signers
                .iter()
                .map(|s| world.address(s))
                .collect::<Result<std::vec::Vec<_>, _>>()?;
            let invoke = MockAuthInvoke {
                contract: &target,
                fn_name: &scenario.function,
                args: args.clone(),
                sub_invokes: &[],
            };
            let auths: std::vec::Vec<MockAuth> = addrs
                .iter()
                .map(|address| MockAuth {
                    address,
                    invoke: &invoke,
                })
                .collect();
            env.mock_auths(&auths);
        }
    }

    // Measure only the scenario invocation itself.
    match scenario.setup.budget {
        Some(BudgetMode::Unlimited) => env.cost_estimate().budget().reset_unlimited(),
        Some(BudgetMode::Default) | None => env.cost_estimate().budget().reset_default(),
    }

    Ok(Prepared { world, target, args })
}

/// Runs one scenario in a brand-new environment and reports what happened,
/// as seen by the host (i.e. the error recorded in diagnostic events).
pub fn run_scenario(catalog: &Catalog, scenario: &Scenario) -> Result<Observation, RunError> {
    let Prepared { world, target, args } = prepare(catalog, scenario)?;
    let env = world.env.clone();
    Ok(match world.invoke(&target, &scenario.function, args) {
        Ok(val) => {
            let return_value = match &scenario.expected {
                Expected::Success {
                    return_value: Some(expected),
                } => Some(decode_like(&env, val, expected).unwrap_or(ArgValue::String {
                    value: format!("<unexpected type: {val:?}>"),
                })),
                _ => None,
            };
            Observation::Success { return_value }
        }
        Err(error) => Observation::Failure {
            error: format!("{error:?}"),
        },
    })
}

/// What a calling contract would see from the same invocation when using
/// `try_invoke_contract` (the host's `try_call`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallerObservation {
    Success,
    /// The error value handed back to the caller.
    Caught(String),
    /// The error was not recoverable; the caller is aborted too.
    Uncatchable,
}

/// Runs the scenario through `Env::try_invoke_contract`, mirroring what a
/// calling contract observes.
pub fn run_scenario_as_caller(catalog: &Catalog, scenario: &Scenario) -> Result<CallerObservation, RunError> {
    let Prepared { world, target, args } = prepare(catalog, scenario)?;
    let env = world.env.clone();
    let func = Symbol::new(&env, &scenario.function);
    // Non-recoverable errors make `try_invoke_contract` panic (the SDK
    // unwraps the host's `Err`), which is how "uncatchable" manifests here.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        env.try_invoke_contract::<Val, Error>(&target, &func, args)
    }));
    Ok(match result {
        Ok(Ok(_)) => CallerObservation::Success,
        Ok(Err(Ok(e))) => CallerObservation::Caught(format!("{e:?}")),
        Ok(Err(Err(e))) => CallerObservation::Caught(format!("{e:?}")),
        Err(_) => CallerObservation::Uncatchable,
    })
}

/// Compares observations against the scenario's expectation. Returns a
/// human-readable mismatch description, or `None` if they agree.
///
/// `caller` is only checked for failure scenarios; pass `None` to skip it.
pub fn check(
    scenario: &Scenario,
    observed: &Observation,
    caller: Option<&CallerObservation>,
) -> Option<String> {
    let host_ok = match (&scenario.expected, observed) {
        (Expected::Success { return_value: exp }, Observation::Success { return_value: got }) => {
            exp.is_none() || exp == got
        }
        (Expected::Failure { error: exp, .. }, Observation::Failure { error: got }) => exp == got,
        _ => false,
    };
    if !host_ok {
        return Some(format!(
            "scenario `{}`: expected {:?}, observed {observed}",
            scenario.id, scenario.expected
        ));
    }
    if let (Expected::Failure { caller_observes, .. }, Some(caller)) = (&scenario.expected, caller) {
        let expected = match caller_observes {
            Some(e) => CallerObservation::Caught(e.clone()),
            None => CallerObservation::Uncatchable,
        };
        if *caller != expected {
            return Some(format!(
                "scenario `{}`: caller expected to observe {expected:?}, observed {caller:?}",
                scenario.id
            ));
        }
    }
    None
}
