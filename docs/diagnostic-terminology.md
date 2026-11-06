# Diagnostic terminology

This document defines the vocabulary shared by all three SorobanLens
repositories. The backend's error analyzer, the frontend's result views and
this repository's fixtures **must use these exact identifiers**. If you change
something here, change it everywhere (and bump `schema_version` in
`fixtures/scenarios.json` if the format changes).

## Failure categories

| Category (`diagnosis.category`) | Meaning | Exercised by fixtures here |
|---|---|---|
| `none` | The invocation succeeded. Used only for success scenarios. | ✅ |
| `authorization_failure` | A required authorization was missing, came from the wrong address, or the contract's own permission check rejected the caller. | ✅ |
| `invalid_arguments` | Arguments had the wrong type, or the contract rejected their values. | ✅ |
| `contract_execution_failure` | Contract logic failed at runtime: a contract error, a panic/trap, arithmetic failure, or a missing storage entry. | ✅ |
| `resource_limit_exceeded` | CPU/memory budget or another resource limit was exceeded. | ✅ |
| `missing_contract_reference` | The invoked or called contract does not exist at that address, or has no function with that name. | ✅ |
| `simulation_failure` | Transaction simulation (`simulateTransaction`) failed before submission. | ❌ backend/RPC-level |
| `network_rpc_error` | The RPC endpoint was unreachable, timed out or returned an error. | ❌ backend/RPC-level |
| `insufficient_information` | Not enough data was available to diagnose the failure with confidence. | ❌ backend-level |

The last three describe tooling and network conditions that cannot be produced
by a contract invocation alone. The backend repository owns their fixtures.

## Evidence levels

SorobanLens keeps what it *knows* separate from what it *suspects*. Each
fixture scenario has fields that line up with these levels:

| Evidence level | Meaning | Fixture field |
|---|---|---|
| **Confirmed** | Observed directly: the exact error value the host produced. | `expected.error`, `expected.contract_error` |
| **Likely cause** | A plausible explanation consistent with the confirmed error. Not proven for any specific transaction. | `diagnosis.likely_causes` |
| **Suggested action** | A debugging step that helps confirm or fix one of the likely causes. | `diagnosis.suggested_fixes` |
| **Insufficient information** | The confirmed data cannot tell apart causes that need different fixes. | category `insufficient_information` (backend) |

A diagnosis must never present a likely cause as confirmed. For example,
`Error(WasmVm, InvalidAction)` confirms that the contract *trapped*. It does
**not** confirm that an overflow happened, even if overflow is the most common
reason.

## Anatomy of a Soroban error

Soroban errors (`ScError`) have a **type** and a **code**, and are shown as
`Error(<Type>, <Code>)`:

- **Types:** `Contract`, `WasmVm`, `Context`, `Storage`, `Object`, `Crypto`,
  `Events`, `Budget`, `Value`, `Auth`.
- **Codes** (for every type except `Contract`): `ArithDomain`, `IndexBounds`,
  `InvalidInput`, `MissingValue`, `ExistingValue`, `ExceededLimit`,
  `InvalidAction`, `InternalError`, `UnexpectedType`, `UnexpectedSize`.
- **Contract errors** carry a contract-defined `u32` instead: `Error(Contract, #3)`.
  The number only means something alongside that contract's
  `#[contracterror]` enum. Each fixture contract's table is in
  `fixtures/scenarios.json` → `contracts.<name>.errors`.

## Verified behaviour that matters for diagnosis

These findings come from running the fixtures (`cargo test`) against
soroban-sdk / soroban-env-host **29.0.1**, and from reading the host source
where noted. Each one shapes how the backend has to analyze failures.

1. **Granular host errors are hidden from calling contracts.** When a contract
   catches a failed sub-call with `try_invoke_contract`, every non-contract
   error becomes `Error(Context, InvalidAction)`. Contract errors keep their
   code. The host source (`soroban-env-host/src/host.rs`, `try_call`) states
   this is deliberate and that *"the granular error codes are still observable
   with diagnostic events."*
   **→ The backend should read diagnostic events, not just the top-level
   result, to get the real error.** Fixture field: `expected.caller_observes`.

2. **Missing authorization and the wrong signer look the same.** Both produce
   `Error(Auth, InvalidAction)`. Telling them apart needs the transaction's
   authorization entries or the diagnostic events.

3. **Host-level and contract-level authorization failures are different.**
   `require_auth()` failures are `Error(Auth, …)`. A contract's own role check
   is a contract error (e.g. `auth-guard`'s `NotAdmin` = `Error(Contract, #1)`).
   Both belong to `authorization_failure`, but the right fixes are different.

4. **Panics carry no cause.** A bare Rust panic (overflow with
   `overflow-checks`, `unwrap()` on `None`, and so on) becomes
   `Error(WasmVm, InvalidAction)`. So does passing an argument of the **wrong
   type**: the SDK's generated entry point traps while converting it. From the
   error code alone these cases look identical. `panic_with_error!` keeps a
   contract code and is much easier to diagnose.

5. **Budget exhaustion cannot be caught.** `Error(Budget, ExceededLimit)` (and
   `Error(Storage, ExceededLimit)`) are non-recoverable. A calling contract
   cannot catch them (`caller_observes: null`), and the whole transaction fails.

6. **Calling a missing contract** produces `Error(Storage, MissingValue)`.

7. **Calling a missing function differs between native and Wasm execution.**
   Native test registration gives `Error(Context, MissingValue)`. A Wasm
   contract gives `Error(WasmVm, MissingValue)` (`soroban-env-host/src/vm.rs`).
   The fixture records both (`wasm_error`). The Wasm value is taken from the
   source code and has not been executed yet.

## Not yet verified

These have **not** been checked yet. Do not rely on them until a fixture or
test confirms them:

- Which of the errors above can be retrieved for a *submitted, failed*
  transaction via Stellar RPC (`getTransaction`), and in what form. This
  includes whether diagnostic events are returned, which depends on RPC/core
  configuration.
- What `simulateTransaction` returns for each scenario.
- How `stellar-cli` reports each scenario. For example, it may refuse to submit
  a transaction whose authorization it cannot sign. That would turn an
  on-chain `authorization_failure` into a client-side `simulation_failure`.

Getting real testnet transaction hashes for each scenario is the top priority
for the next milestone (see the README roadmap).
