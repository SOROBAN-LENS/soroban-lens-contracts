//! Serde model of `fixtures/scenarios.json`.
//!
//! The catalog is the integration point with `soroban-lens-backend`: the
//! backend's diagnostic-engine tests consume the same file. Field names are
//! therefore part of a public contract; see `fixtures/README.md` and
//! `fixtures/scenarios.schema.json` before changing anything here.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Top-level fixture catalog.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    /// Bumped on breaking changes to this format.
    pub schema_version: u32,
    /// soroban-sdk version the expectations were verified against.
    pub soroban_sdk_version: String,
    /// Per-contract deployment info and error-code tables, keyed by contract
    /// name (the crate name under `contracts/`).
    pub contracts: BTreeMap<String, ContractInfo>,
    pub scenarios: Vec<Scenario>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractInfo {
    pub description: String,
    /// Arguments passed to `__constructor` at deploy time.
    #[serde(default)]
    pub constructor_args: Vec<ArgValue>,
    /// Contract error code (as decimal string) -> variant name.
    pub errors: BTreeMap<String, String>,
}

/// A single reproducible invocation with a known outcome.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    /// Stable, unique identifier: `<contract>/<function>/<case>`.
    pub id: String,
    pub title: String,
    pub contract: String,
    pub function: String,
    #[serde(default)]
    pub args: Vec<ArgValue>,
    pub auth: AuthMode,
    #[serde(default)]
    pub setup: Setup,
    pub expected: Expected,
    pub diagnosis: Diagnosis,
}

/// How authorization is provided for the scenario's root invocation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthMode {
    /// No authorization entries at all.
    None,
    /// Every `require_auth` succeeds (`Env::mock_all_auths`).
    MockAll,
    /// Only the listed actors authorize the root invocation with exactly the
    /// scenario's `args`.
    Signers(Vec<String>),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Setup {
    /// Ledger sequence number to set before invoking.
    #[serde(default)]
    pub ledger_sequence: Option<u32>,
    /// `"default"` (network-like limits) or `"unlimited"`. Defaults to the
    /// soroban-sdk test environment default.
    #[serde(default)]
    pub budget: Option<BudgetMode>,
    /// Calls executed (with all auths mocked) before the scenario call. Each
    /// must succeed.
    #[serde(default)]
    pub calls: Vec<SetupCall>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BudgetMode {
    Default,
    Unlimited,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetupCall {
    pub function: String,
    #[serde(default)]
    pub args: Vec<ArgValue>,
}

/// A typed Soroban argument or return value in JSON form.
///
/// 64- and 128-bit integers are JSON strings so no precision is lost in
/// JavaScript consumers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ArgValue {
    Void,
    Bool {
        value: bool,
    },
    U32 {
        value: u32,
    },
    U64 {
        value: String,
    },
    I128 {
        value: String,
    },
    String {
        value: String,
    },
    Symbol {
        value: String,
    },
    /// A named address. Actors (`admin`, `alice`, `mallory`, ...) are freshly
    /// generated per scenario run; `@<contract-name>` resolves to that
    /// deployed contract; `unregistered_contract` is a contract address with
    /// nothing deployed at it.
    Address {
        actor: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum Expected {
    Success {
        /// Expected return value. Omitted when the value is not a simple type
        /// (e.g. a struct) and only success matters.
        #[serde(default)]
        return_value: Option<ArgValue>,
    },
    Failure {
        /// `Debug` form of the host error, e.g. `Error(Auth, InvalidAction)`
        /// or `Error(Contract, #1)`. This is the form shown in diagnostic
        /// events and `stellar` CLI output.
        error: String,
        /// Contract error variant name; set iff the error type is `Contract`.
        #[serde(default)]
        contract_error: Option<String>,
        /// What a *calling contract* using `try_invoke_contract` observes.
        /// Non-contract recoverable errors are narrowed by the host to
        /// `Error(Context, InvalidAction)`; non-recoverable ones (budget)
        /// cannot be caught at all, written as `null`. Verified by the runner.
        caller_observes: Option<String>,
        /// Error expected when the contract runs as **Wasm**, only when it
        /// differs from `error` (which is observed with native registration).
        /// Derived from soroban-env-host source; *not* executed by the runner.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        wasm_error: Option<String>,
    },
}

/// What SorobanLens should conclude about this scenario. These fields are
/// the ground truth used to test the backend's error analyzer.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Diagnosis {
    /// One of [`CATEGORIES`].
    pub category: String,
    pub summary: String,
    #[serde(default)]
    pub likely_causes: Vec<String>,
    #[serde(default)]
    pub suggested_fixes: Vec<String>,
}

/// The shared SorobanLens failure-category vocabulary. Must stay in sync with
/// `docs/diagnostic-terminology.md` and the backend's `ErrorCategory` enum.
pub const CATEGORIES: &[&str] = &[
    "none",
    "authorization_failure",
    "invalid_arguments",
    "contract_execution_failure",
    "resource_limit_exceeded",
    "missing_contract_reference",
    "simulation_failure",
    "network_rpc_error",
    "insufficient_information",
];
