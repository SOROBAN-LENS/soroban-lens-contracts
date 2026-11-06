//! # scenario-runner
//!
//! Loads the SorobanLens fixture catalog (`fixtures/scenarios.json`), executes
//! every scenario against the example contracts in a soroban-sdk test
//! environment, and verifies that the observed outcome matches the catalog.
//!
//! This is what makes the fixtures trustworthy for `soroban-lens-backend`:
//! an expectation in the catalog that no longer matches real contract
//! behaviour fails CI.

pub mod catalog;
pub mod runner;

use catalog::Catalog;
use soroban_sdk::Error;
use std::path::{Path, PathBuf};

/// Path to the catalog shipped in this repository.
pub fn default_catalog_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/scenarios.json")
}

/// Reads and parses a catalog file.
pub fn load_catalog(path: &Path) -> Result<Catalog, Box<dyn std::error::Error>> {
    let raw = std::fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    Ok(serde_json::from_str(&raw).map_err(|e| format!("parsing {}: {e}", path.display()))?)
}

/// Returns the Rust variant name of contract error `code` for `contract`, as
/// defined by the contract's `#[contracterror]` enum, or `None` if the code is
/// not defined.
pub fn contract_error_name(contract: &str, code: u32) -> Option<String> {
    let err = Error::from_contract_error(code);
    match contract {
        "auth-guard" => auth_guard::AuthGuardError::try_from(err)
            .ok()
            .map(|e| format!("{e:?}")),
        "input-validator" => input_validator::InputValidatorError::try_from(err)
            .ok()
            .map(|e| format!("{e:?}")),
        "execution-failure" => execution_failure::ExecutionFailureError::try_from(err)
            .ok()
            .map(|e| format!("{e:?}")),
        _ => None,
    }
}
