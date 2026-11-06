# Fixtures

`scenarios.json` is the SorobanLens **fixture catalog**: reproducible contract
invocations, each with a **verified** outcome and a ground-truth diagnosis.

- Format: [`scenarios.schema.json`](scenarios.schema.json) (JSON Schema
  2020-12), mirrored by `crates/scenario-runner/src/catalog.rs`.
- Verification: `cargo test -p scenario-runner` runs every scenario against
  the real contracts. See [../docs/verification.md](../docs/verification.md).
- Vocabulary: [../docs/diagnostic-terminology.md](../docs/diagnostic-terminology.md).

## Shape of a scenario

```jsonc
{
  "id": "auth-guard/set_value/missing-auth",      // <contract>/<function>/<case>, stable
  "title": "Set the value with no authorization",
  "contract": "auth-guard",
  "function": "set_value",
  "args": [{ "type": "u32", "value": 42 }],
  "auth": "none",                                 // "none" | "mock_all" | {"signers": ["admin", ...]}
  "setup": { "ledger_sequence": 1000, "budget": "default", "calls": [] },   // optional
  "expected": {
    "outcome": "failure",
    "error": "Error(Auth, InvalidAction)",        // CONFIRMED: host error / diagnostic events
    "caller_observes": "Error(Context, InvalidAction)"  // what a calling contract sees; null = uncatchable
    // "contract_error": "NotAdmin"              // only for Error(Contract, #N)
    // "wasm_error": "..."                       // only if Wasm differs from native (source-derived)
  },
  "diagnosis": {
    "category": "authorization_failure",
    "summary": "...",
    "likely_causes": ["..."],                     // LIKELY, not confirmed
    "suggested_fixes": ["..."]
  }
}
```

### Values

`{"type": ..., ...}` with `type` one of `void`, `bool`, `u32`, `u64`, `i128`,
`string`, `symbol`, `address`. `u64` and `i128` values are **strings**, so
JavaScript consumers do not lose precision.

### Addresses

`{"type": "address", "actor": "<name>"}`:

- plain names (`admin`, `alice`, `mallory`) are freshly generated accounts
  for each run. `admin` is the one passed to constructors.
- `@<contract>` (e.g. `@auth-guard`) is that deployed fixture contract
- `unregistered_contract` is an address with nothing deployed at it

## Adding a scenario

1. Add the entry to `scenarios.json` (keep entries grouped by contract).
2. Run `cargo run -q -p scenario-runner`. If your expectation is wrong, the
   runner prints what actually happened. Investigate the difference; do not
   just copy the output without understanding it.
3. Run `cargo test --workspace`.
4. If the scenario shows new behaviour, document it in
   `docs/diagnostic-terminology.md`.
