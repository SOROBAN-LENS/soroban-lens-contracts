# Contributing to soroban-lens-contracts

Thanks for helping make Soroban failures easier to understand! This
repository holds **fixture contracts** and a **verified scenario catalog**, so
the most important rule is:

> **Every expectation must be true.** If you can't verify a behaviour, say so
> explicitly. Never write down an error you haven't observed (or derived from
> source, clearly labeled as such).

## Getting set up

```bash
rustup show                     # rust-toolchain.toml installs stable + wasm32v1-none
cargo test --workspace
cargo run -q -p scenario-runner
```

Install [stellar-cli](https://developers.stellar.org/docs/tools/cli) ≥ 25.2.0
if you need to build deployable Wasm (`stellar contract build`).

## Ways to contribute

- **New failure scenarios.** Open a "New failure scenario" issue first, so we
  can agree on the category and expected behaviour.
- **Testnet verification.** Run the commands in `docs/contracts/*.md` and
  report the actual output.
- **Diagnosis text.** Make `likely_causes` / `suggested_fixes` clearer and
  more accurate.
- **Docs** and **tooling** improvements.

## Workflow

1. Fork, then branch from `main` (`feat/…`, `fix/…`, `docs/…`).
2. Make your change, with tests.
3. Run `make check` (fmt, clippy `-D warnings`, all tests).
4. Open a pull request using the template.

### Rules for contracts

- Keep each contract small: a failure should have exactly **one** cause.
- `#[contracterror]` codes are a public contract: **never renumber or reuse
  them**. Add new variants with new codes.
- Prefer returning `Result<_, YourError>` or using `panic_with_error!`. Add a
  bare panic only when the panic itself is what's being shown, and document it.
- No unbounded loops, no real-value token logic, no privileged functions
  without `require_auth`.
- Add unit tests in `src/test.rs` for every success and failure path.

### Rules for the catalog (`fixtures/scenarios.json`)

- Scenario `id`s are stable (`<contract>/<function>/<case>`). Don't rename or
  reuse them.
- Category must come from `docs/diagnostic-terminology.md`.
- `expected` must match what the scenario runner observes. If it doesn't,
  find out *why* before changing anything.
- Things learned from reading source rather than running code go in
  clearly-labeled fields (e.g. `wasm_error`) or docs, never in `error`.
- Breaking format changes require bumping `schema_version` and updating
  `scenarios.schema.json`, `catalog.rs` and `fixtures/README.md` together.

### Commit messages

Use short imperative summaries, e.g. `Add storage archival scenario`. Reference
issues with `Fixes #123` where relevant.

## Code of Conduct

By participating you agree to follow our [Code of Conduct](CODE_OF_CONDUCT.md).
