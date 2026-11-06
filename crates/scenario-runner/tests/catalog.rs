//! Verifies `fixtures/scenarios.json` against the real contracts.
//!
//! If one of these tests fails, either a contract's behaviour changed or the
//! catalog is wrong. Fix whichever is incorrect. Never weaken the check.

use scenario_runner::catalog::{Catalog, Expected, CATEGORIES};
use scenario_runner::runner::{check, run_scenario, run_scenario_as_caller};
use scenario_runner::{contract_error_name, default_catalog_path, load_catalog};
use std::collections::BTreeSet;

fn catalog() -> Catalog {
    load_catalog(&default_catalog_path()).expect("fixtures/scenarios.json loads")
}

/// Parses `Error(Contract, #N)` into `N`.
fn contract_code(error: &str) -> Option<u32> {
    error
        .strip_prefix("Error(Contract, #")?
        .strip_suffix(')')?
        .parse()
        .ok()
}

#[test]
fn every_scenario_matches_observed_behaviour() {
    let catalog = catalog();
    let mut failures = Vec::new();
    for s in &catalog.scenarios {
        let observed = match run_scenario(&catalog, s) {
            Ok(o) => o,
            Err(e) => {
                failures.push(format!("{}: could not run: {e}", s.id));
                continue;
            }
        };
        let caller = match s.expected {
            Expected::Failure { .. } => match run_scenario_as_caller(&catalog, s) {
                Ok(c) => Some(c),
                Err(e) => {
                    failures.push(format!("{}: could not run as caller: {e}", s.id));
                    continue;
                }
            },
            Expected::Success { .. } => None,
        };
        if let Some(m) = check(s, &observed, caller.as_ref()) {
            failures.push(m);
        }
    }
    assert!(
        failures.is_empty(),
        "{} mismatches:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn scenario_ids_are_unique_and_well_formed() {
    let catalog = catalog();
    let mut seen = BTreeSet::new();
    for s in &catalog.scenarios {
        assert!(seen.insert(&s.id), "duplicate scenario id `{}`", s.id);
        let prefix = format!("{}/{}/", s.contract, s.function);
        assert!(
            s.id.starts_with(&prefix) && s.id.len() > prefix.len(),
            "scenario id `{}` must look like `{prefix}<case>`",
            s.id
        );
    }
}

#[test]
fn categories_use_the_shared_vocabulary() {
    let catalog = catalog();
    for s in &catalog.scenarios {
        assert!(
            CATEGORIES.contains(&s.diagnosis.category.as_str()),
            "{}: unknown category `{}`",
            s.id,
            s.diagnosis.category
        );
        let is_success = matches!(s.expected, Expected::Success { .. });
        assert_eq!(
            is_success,
            s.diagnosis.category == "none",
            "{}: category `none` is reserved for (and required by) success scenarios",
            s.id
        );
        if !is_success {
            assert!(
                !s.diagnosis.suggested_fixes.is_empty(),
                "{}: failures need suggested fixes",
                s.id
            );
        }
    }
}

#[test]
fn contract_error_tables_match_rust_enums() {
    let catalog = catalog();
    for (name, info) in &catalog.contracts {
        for (code, variant) in &info.errors {
            let code: u32 = code
                .parse()
                .unwrap_or_else(|_| panic!("{name}: bad code `{code}`"));
            assert_eq!(
                contract_error_name(name, code).as_deref(),
                Some(variant.as_str()),
                "{name}: catalog says code {code} is `{variant}`"
            );
        }
        // No enum variant may be missing from the table (codes are dense from 1).
        let next = info.errors.len() as u32 + 1;
        assert_eq!(
            contract_error_name(name, next),
            None,
            "{name}: Rust enum defines code {next}, which the catalog's error table is missing"
        );
    }
}

#[test]
fn contract_failures_name_the_right_variant() {
    let catalog = catalog();
    for s in &catalog.scenarios {
        if let Expected::Failure {
            error,
            contract_error,
            ..
        } = &s.expected
        {
            let expected_name =
                contract_code(error).map(|c| catalog.contracts[&s.contract].errors[&c.to_string()].clone());
            assert_eq!(
                contract_error, &expected_name,
                "{}: contract_error must be set exactly when the error is a contract error, and match the code",
                s.id
            );
        }
    }
}

#[test]
fn every_contract_error_code_has_a_scenario() {
    let catalog = catalog();
    for (name, info) in &catalog.contracts {
        let covered: BTreeSet<String> = catalog
            .scenarios
            .iter()
            .filter(|s| &s.contract == name)
            .filter_map(|s| match &s.expected {
                Expected::Failure { error, .. } => contract_code(error).map(|c| c.to_string()),
                _ => None,
            })
            .collect();
        for code in info.errors.keys() {
            assert!(
                covered.contains(code),
                "{name}: contract error #{code} has no scenario"
            );
        }
    }
}

#[test]
fn every_category_except_network_level_ones_is_exercised() {
    // `simulation_failure`, `network_rpc_error` and `insufficient_information`
    // describe RPC/tooling conditions that cannot be produced by a contract
    // invocation alone; they are the backend's responsibility to fixture.
    let catalog = catalog();
    let used: BTreeSet<&str> = catalog
        .scenarios
        .iter()
        .map(|s| s.diagnosis.category.as_str())
        .collect();
    for c in [
        "none",
        "authorization_failure",
        "invalid_arguments",
        "contract_execution_failure",
        "resource_limit_exceeded",
        "missing_contract_reference",
    ] {
        assert!(used.contains(c), "no scenario exercises category `{c}`");
    }
}

#[test]
fn checker_detects_a_wrong_expectation() {
    // Guard against a runner that passes everything: corrupt one expectation
    // and confirm the mismatch is reported.
    let catalog = catalog();
    let mut s = catalog
        .scenarios
        .iter()
        .find(|s| s.id == "auth-guard/set_value/missing-auth")
        .expect("scenario exists")
        .clone();
    if let Expected::Failure { error, .. } = &mut s.expected {
        *error = "Error(Contract, #1)".to_string();
    }
    let observed = run_scenario(&catalog, &s).expect("runs");
    assert!(check(&s, &observed, None).is_some());

    // And a wrong caller-side expectation is caught too.
    let mut s = catalog
        .scenarios
        .iter()
        .find(|s| s.id == "execution-failure/hash_chain/budget-exceeded")
        .expect("scenario exists")
        .clone();
    if let Expected::Failure { caller_observes, .. } = &mut s.expected {
        *caller_observes = Some("Error(Context, InvalidAction)".to_string());
    }
    let observed = run_scenario(&catalog, &s).expect("runs");
    let caller = run_scenario_as_caller(&catalog, &s).expect("runs");
    assert!(check(&s, &observed, Some(&caller)).is_some());
}
