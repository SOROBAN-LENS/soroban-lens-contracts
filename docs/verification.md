# What is (and is not) verified

SorobanLens fixtures are only useful if they are true. This page lists exactly
how each claim in this repository was checked.

## Verified automatically on every `cargo test`

| Claim | How |
|---|---|
| Each contract function behaves as its unit tests describe | `cargo test -p <contract>` uses the generated typed clients, `env.auths()` and `env.events()` |
| Every scenario in `fixtures/scenarios.json` produces exactly its `expected.error` / `return_value` | `crates/scenario-runner/tests/catalog.rs` runs each scenario in a fresh `Env` and calls `Host::call` directly to get the full host error |
| Every failure's `caller_observes` value is what a calling contract sees | Same test, re-run through `Env::try_invoke_contract` (the host's `try_call`). `null` is confirmed by the call aborting |
| Catalog error tables match the Rust `#[contracterror]` enums exactly | `contract_error_tables_match_rust_enums` |
| Every contract error code is covered by at least one scenario | `every_contract_error_code_has_a_scenario` |
| The checker rejects wrong expectations (it does not pass everything) | `checker_detects_a_wrong_expectation` |
| The catalog conforms to `fixtures/scenarios.schema.json` | Checked manually with Python `jsonschema` (Draft 2020-12). **Not yet automated in CI** |

## Native vs Wasm execution

The scenario runner registers contracts **natively**: the contract code is
compiled into the test binary and the soroban-env-host runs it directly.

This faithfully reproduces:

- contract logic, storage, TTLs and events
- authorization (`require_auth`, mocked authorization entries)
- host-function costs (e.g. SHA-256, storage), so budget exhaustion works
- error values. A Rust panic is mapped to `Error(WasmVm, InvalidAction)`, the
  same error a Wasm trap produces.

It does **not** reproduce:

- Wasm instruction metering. Native budgets *underestimate* CPU cost, so the
  exact iteration count at which `hash_chain` runs out differs from Wasm
  (the 1,000,000-iteration scenario fails either way).
- Wasm-specific error paths. The one known difference is calling a missing
  function (`wasm_error` in the catalog). It was found by reading
  `soroban-env-host/src/vm.rs` and has **not** been executed.

## Wasm build

- `soroban-sdk` 29 refuses to build Wasm unless invoked by `stellar contract
  build` (stellar-cli ≥ 25.2.0), because the CLI post-processes the contract
  spec ("spec shaking").
- During development, all three contracts were compiled for `wasm32v1-none`
  with the release profile, as a **compile-only** check (12–16 KB each). That
  run set `SOROBAN_SDK_BUILD_SYSTEM_SUPPORTS_SPEC_SHAKING_V2` by hand, so its
  output was **not** a deployable artifact and was thrown away.
- The CI workflow builds with `stellar contract build`. That job has not run
  yet at the time of writing.

## Not verified yet

- **Deployment and invocation on testnet.** `scripts/deploy-testnet.sh` and the
  `stellar` commands in `docs/contracts/*.md` follow the documented
  stellar-cli v25+ syntax but have not been run end-to-end. Outputs shown in
  those docs are *expected*, not captured.
- **RPC visibility.** No claim is made yet about which error details Stellar
  RPC returns for a failed transaction. See
  [diagnostic-terminology.md](diagnostic-terminology.md#not-yet-verified).
