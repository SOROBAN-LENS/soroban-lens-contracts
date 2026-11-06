//! `scenario-runner` CLI.
//!
//! ```text
//! cargo run -p scenario-runner                 # human-readable report
//! cargo run -p scenario-runner -- --json       # machine-readable report
//! cargo run -p scenario-runner -- --catalog path/to/scenarios.json
//! ```
//!
//! Exits non-zero if any scenario fails to run or does not match its
//! expectation.

use scenario_runner::catalog::Expected;
use scenario_runner::runner::{check, run_scenario, run_scenario_as_caller};
use scenario_runner::{default_catalog_path, load_catalog};
use serde::Serialize;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Serialize)]
struct ReportEntry<'a> {
    id: &'a str,
    category: &'a str,
    passed: bool,
    observed: String,
    detail: Option<String>,
}

fn main() -> ExitCode {
    let mut json = false;
    let mut catalog_path = default_catalog_path();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--json" => json = true,
            "--catalog" => match args.next() {
                Some(p) => catalog_path = PathBuf::from(p),
                None => {
                    eprintln!("--catalog requires a path");
                    return ExitCode::from(2);
                }
            },
            "-h" | "--help" => {
                println!("usage: scenario-runner [--json] [--catalog <path>]");
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("unknown argument `{other}` (try --help)");
                return ExitCode::from(2);
            }
        }
    }

    let catalog = match load_catalog(&catalog_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };

    // Bare panics inside natively-registered contracts are expected for some
    // scenarios; silence the default hook so the report stays readable.
    std::panic::set_hook(Box::new(|_| {}));

    let mut entries = Vec::new();
    for s in &catalog.scenarios {
        let result = run_scenario(&catalog, s).and_then(|obs| {
            let caller = match s.expected {
                Expected::Failure { .. } => Some(run_scenario_as_caller(&catalog, s)?),
                Expected::Success { .. } => None,
            };
            Ok((obs, caller))
        });
        let (passed, observed, detail) = match result {
            Ok((obs, caller)) => {
                let mismatch = check(s, &obs, caller.as_ref());
                (mismatch.is_none(), obs.to_string(), mismatch)
            }
            Err(e) => (false, "not run".to_string(), Some(e.to_string())),
        };
        entries.push(ReportEntry {
            id: &s.id,
            category: &s.diagnosis.category,
            passed,
            observed,
            detail,
        });
    }
    let _ = std::panic::take_hook();

    let failed = entries.iter().filter(|e| !e.passed).count();
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&entries).expect("report serializes")
        );
    } else {
        for e in &entries {
            let mark = if e.passed { "PASS" } else { "FAIL" };
            println!("{mark}  {:<55} {:<28} {}", e.id, e.category, e.observed);
            if let Some(d) = &e.detail {
                println!("      {d}");
            }
        }
        println!(
            "\n{} scenarios, {} passed, {} failed",
            entries.len(),
            entries.len() - failed,
            failed
        );
    }
    if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
