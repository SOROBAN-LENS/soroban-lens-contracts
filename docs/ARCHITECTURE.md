# Architecture: soroban-lens-contracts

## Role in SorobanLens

SorobanLens is split across three repositories:

```
┌──────────────────────────┐   REST (JSON)   ┌───────────────────────────────┐   JSON-RPC   ┌──────────────┐
│ soroban-lens-frontend    │ ──────────────▶ │ soroban-lens-backend          │ ───────────▶ │ Stellar RPC  │
│ React · TypeScript       │                 │ Axum API                      │              │ (testnet /   │
│ analyzer UI, results     │ ◀────────────── │  ├ stellar-client             │ ◀─────────── │  mainnet)    │
└──────────────────────────┘ DiagnosticReport│  ├ diagnostic-engine          │              └──────▲───────┘
                                             │  └ error-analyzer (rules)     │                     │
                                             └──────────────▲────────────────┘                     │
                                                            │ fixtures/scenarios.json              │ deploy &
                                                            │ (test ground truth)                  │ invoke
                                             ┌──────────────┴────────────────┐                     │
                                             │ soroban-lens-contracts (here) │ ────────────────────┘
                                             │ fixture contracts + catalog   │
                                             └───────────────────────────────┘
```

This repository is the **source of ground truth**: real contracts with
known, verified failure behaviour. It is never called at runtime by the
backend or frontend.

## Integration points

| Consumer | What it uses | How |
|---|---|---|
| backend `error-analyzer` tests | `fixtures/scenarios.json`: `expected.error` → `diagnosis.category` / text | Vendor or git-submodule the file; assert the analyzer's output for each `expected.error` matches `diagnosis.category` |
| backend `diagnostic-engine` | Contract error tables (`contracts.<name>.errors`) | Map `Error(Contract, #N)` to a variant name for the known fixture contracts |
| backend RPC integration tests *(planned)* | Testnet contract IDs + failed transaction hashes per scenario | `deployments/testnet.json` (planned) |
| frontend *(planned)* | Example transaction hashes for the demo/docs page | Same planned deployment record |
| all repos | Category names, evidence levels | [diagnostic-terminology.md](diagnostic-terminology.md) |

### Contract with the backend

The backend repository should pin a specific commit or tag of
`fixtures/scenarios.json`, and treat these as stable:

- `schema_version`: breaking format changes bump it
- scenario `id`s: never reused for a different scenario
- contract error codes: never renumbered (enforced by tests here)
- category identifiers: listed in `CATEGORIES` in
  `crates/scenario-runner/src/catalog.rs` and in the JSON schema

## Repository layout

```
contracts/
  auth-guard/          authorization failures (host-level vs contract-level)
  input-validator/     one contract error per validation rule
  execution-failure/   panics, overflow, missing storage/contract, budget
crates/
  scenario-runner/     loads the catalog, runs every scenario, verifies outcomes
fixtures/
  scenarios.json       the catalog (consumed by the backend)
  scenarios.schema.json
docs/                  terminology, verification notes, per-contract guides
scripts/               testnet deployment helper
```

### Why a data-driven runner instead of only unit tests?

Unit tests (in each contract's `src/test.rs`) check contract behaviour through
typed clients. The **runner** checks the *catalog*: the JSON file other
repositories rely on. If someone edits a contract or the catalog and they no
longer agree, CI fails. The runner calls `Host::call` directly, because the
SDK's `try_*` helpers hide granular host errors (see
[diagnostic-terminology.md](diagnostic-terminology.md#verified-behaviour-that-matters-for-diagnosis)).

### Design decisions

- **Constructors (`__constructor`)** instead of `initialize` functions, so
  contracts are never left half-initialized and admin setup cannot be raced.
- **Stable error codes:** `#[contracterror]` enums with explicit discriminants
  that tests check against the catalog.
- **TTL extension on write** for instance and persistent storage, following
  Soroban state-archival guidance.
- **Events via `#[contractevent]`**, the SDK's typed event API.
- **No token or financial logic.** The contracts are intentionally small, so a
  failure always has exactly one cause.
