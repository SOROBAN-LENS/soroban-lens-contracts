# soroban-lens-contracts

> **SorobanLens**: making Soroban smart contract failures understandable.

Example Soroban smart contracts that fail **on purpose, predictably**, plus a
verified fixture catalog. SorobanLens uses them as ground truth to test its
diagnostic engine.

> **Status: early development (milestone 1 of ~3).** Not production software,
> and the contracts are not meant to be reused in production. See
> [Current status](#current-status) for exactly what works and what has been
> verified.

---

## Contents

- [Role in SorobanLens](#role-in-sorobanlens)
- [What's inside](#whats-inside)
- [Quick start](#quick-start)
- [Testing](#testing)
- [Configuration](#configuration)
- [Current status](#current-status)
- [Roadmap](#roadmap)
- [Contributing](#contributing)

## Role in SorobanLens

SorobanLens is three repositories:

| Repository | Role |
|---|---|
| `soroban-lens-frontend` | React/TypeScript dashboard: submit a transaction hash, view the diagnosis |
| `soroban-lens-backend` | Rust/Axum API, Stellar RPC client, diagnostic engine, rule-based error analyzer |
| **`soroban-lens-contracts`** (this repo) | Fixture contracts + a verified catalog of failure scenarios used to test the backend |

A diagnostic tool is only as good as its test cases. This repository provides
contracts whose failures are **known exactly**, so the backend's analyzer can
be checked against real behaviour instead of made-up error strings. See
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the integration points.

## What's inside

| Contract | Demonstrates | Docs |
|---|---|---|
| [`auth-guard`](contracts/auth-guard) | Missing authorization, wrong signer, partial multi-party authorization, contract-level role check | [docs](docs/contracts/auth-guard.md) |
| [`input-validator`](contracts/input-validator) | One stable contract error per validation rule; wrong argument types | [docs](docs/contracts/input-validator.md) |
| [`execution-failure`](contracts/execution-failure) | Handled vs. bare panics, overflow, missing storage, missing contract/function, budget exhaustion | [docs](docs/contracts/execution-failure.md) |

| Other components | |
|---|---|
| [`fixtures/scenarios.json`](fixtures/scenarios.json) | **36 scenarios**: each has the invocation, the exact expected error, and a ground-truth diagnosis (category, likely causes, suggested fixes) |
| [`crates/scenario-runner`](crates/scenario-runner) | Runs every scenario against the real contracts and fails if the catalog and the contracts disagree |
| [`docs/diagnostic-terminology.md`](docs/diagnostic-terminology.md) | The shared category / evidence vocabulary, plus **verified findings** about how Soroban reports errors |

### Technologies

- Rust (stable, edition 2021), `wasm32v1-none` target
- [`soroban-sdk`](https://crates.io/crates/soroban-sdk) **29.0.1** (latest stable at the time of writing)
- [`stellar-cli`](https://github.com/stellar/stellar-cli) ≥ 25.2.0 to build deployable Wasm
- `serde` / `serde_json` for the fixture catalog

### A taste of what the fixtures reveal

Running the catalog showed several things that directly affect how
SorobanLens must analyze failures:

- A contract that catches a failed sub-call sees **every** host error as
  `Error(Context, InvalidAction)`. The real error (e.g. `Error(Auth, InvalidAction)`)
  is only visible in **diagnostic events**.
- Missing authorization and a wrong signer give the **same** error.
- A bare panic and an argument of the wrong type **both** give
  `Error(WasmVm, InvalidAction)`. The code alone cannot tell them apart.
- Budget exhaustion **cannot be caught** by a calling contract.

Details: [docs/diagnostic-terminology.md](docs/diagnostic-terminology.md#verified-behaviour-that-matters-for-diagnosis).

## Quick start

### Prerequisites

```bash
# Rust (the repo's rust-toolchain.toml selects stable + wasm32v1-none)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Optional, needed only to build deployable Wasm / deploy:
# stellar-cli >= 25.2.0. See https://developers.stellar.org/docs/tools/cli
```

### Build, test, run the scenarios

```bash
git clone https://github.com/soroban-lens/soroban-lens-contracts
cd soroban-lens-contracts

cargo test --workspace            # unit tests + fixture verification
cargo run -q -p scenario-runner   # human-readable scenario report
```

Example output (abridged):

```
PASS  auth-guard/set_value/missing-auth          authorization_failure        failure: Error(Auth, InvalidAction)
PASS  auth-guard/set_value_as/not-admin          authorization_failure        failure: Error(Contract, #1)
PASS  execution-failure/hash_chain/budget-exceeded resource_limit_exceeded    failure: Error(Budget, ExceededLimit)
...
36 scenarios, 36 passed, 0 failed
```

Use `cargo run -q -p scenario-runner -- --json` for machine-readable output.

### Build deployable Wasm

```bash
stellar contract build    # → target/wasm32v1-none/release/*.wasm
```

`soroban-sdk` 29 deliberately refuses `cargo build --target wasm32v1-none`.
Only `stellar contract build` produces a correct contract spec.

### Deploy to testnet

```bash
cp .env.example .env
stellar keys generate sorobanlens-admin --network testnet --fund
scripts/deploy-testnet.sh
```

> ⚠️ The deploy script and the testnet commands in `docs/contracts/` have
> **not yet been run end-to-end**. See [docs/verification.md](docs/verification.md).

## Testing

| Command | What it checks |
|---|---|
| `cargo test -p auth-guard` (etc.) | Contract unit tests: return values, errors, recorded authorizations, emitted events |
| `cargo test -p scenario-runner` | Every catalog scenario matches real behaviour (host error **and** the error a calling contract sees), error tables match the Rust enums, every error code is covered, and the checker rejects wrong expectations |
| `make check` | `fmt --check` + `clippy -D warnings` + all tests (what CI runs) |

Current results: **40 tests, all passing** (auth-guard 10, input-validator 8,
execution-failure 14, scenario-runner 8, the last of which runs all 36 catalog
scenarios).

## Configuration

Only the deploy script reads configuration. See [`.env.example`](.env.example).
It holds **no secrets**: signing keys stay in the Stellar CLI keystore and are
referenced by identity name. `.env` is git-ignored.

| Variable | Default | Purpose |
|---|---|---|
| `STELLAR_NETWORK` | `testnet` | Stellar CLI network name |
| `STELLAR_RPC_URL` | `https://soroban-testnet.stellar.org` | RPC endpoint (informational for now) |
| `STELLAR_NETWORK_PASSPHRASE` | testnet passphrase | Network passphrase |
| `STELLAR_ADMIN_IDENTITY` | `sorobanlens-admin` | CLI identity used as deployer/admin |
| `STELLAR_OTHER_IDENTITY` | `sorobanlens-other` | Second identity for wrong-signer scenarios |

## Current status

### Implemented and verified (by `cargo test`)

- [x] Three contracts with stable error codes, typed events, constructors and TTL handling
- [x] 32 contract unit tests
- [x] 36-scenario fixture catalog covering 6 of the 9 categories (the other 3 are RPC/network-level and belong to the backend)
- [x] Data-driven scenario runner (library + CLI + test suite) that checks host errors *and* caller-observed errors
- [x] JSON Schema for the catalog (catalog validated against it manually)
- [x] Shared terminology document with verified findings
- [x] All contracts compile for `wasm32v1-none` (compile-only check; see verification notes)

### Implemented but not yet verified

- [ ] `scripts/deploy-testnet.sh` and the documented `stellar` CLI invocations
- [ ] CI workflow (`.github/workflows/ci.yml`), including the `stellar contract build` job
- [ ] Wasm-specific expectation (`wasm_error`) for calling a missing function, taken from the host source

### Not implemented yet

- Testnet deployment record and real **failed transaction hashes** for each scenario
- Running scenarios against the built Wasm (instead of native registration)
- Recording diagnostic events per scenario in the catalog
- Automated JSON-Schema validation in CI

## Roadmap

The next milestone focuses on connecting fixtures to the **real network**,
which is what the backend needs:

1. Run `stellar contract build` in CI and run the scenario catalog against the Wasm build
2. Deploy to testnet; commit `deployments/testnet.json`
3. Produce one real **failed transaction hash** per failure scenario, recorded in the catalog
4. Record what `getTransaction` / `simulateTransaction` return for each (closes the "Not yet verified" list)
5. Capture the diagnostic events for each scenario in the catalog
6. Add scenarios: storage archival/restoration, cross-contract error propagation, events/limits, `require_auth_for_args`
7. Add JSON-Schema validation to CI

## Contributing

Contributions are welcome. New failure scenarios are especially valuable.
Read [CONTRIBUTING.md](CONTRIBUTING.md) and our
[Code of Conduct](CODE_OF_CONDUCT.md). Report vulnerabilities via
[SECURITY.md](SECURITY.md), not public issues.

Good first contributions:

- Run the testnet commands in `docs/contracts/` and report the real output
- Propose a new failure scenario (there's an issue template for it)
- Improve the `likely_causes` / `suggested_fixes` text in the catalog

## License

[MIT](LICENSE)
