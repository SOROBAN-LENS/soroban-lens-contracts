# auth-guard: authorization failures

**Source:** [`contracts/auth-guard/src/lib.rs`](../../contracts/auth-guard/src/lib.rs)
**Category exercised:** `authorization_failure` (plus one `invalid_arguments`)

Stores an admin address and a `u32` value. It shows the two different ways
authorization can fail:

- **host-level:** `require_auth()` is not satisfied, so the host aborts with
  `Error(Auth, InvalidAction)`
- **contract-level:** the caller authenticated but the contract's own role
  check rejected them, giving a contract error

## Interface

| Function | Auth required | Returns / errors |
|---|---|---|
| `__constructor(admin: Address, initial_value: u32)` | — (deploy time) | |
| `admin() -> Address` | none | |
| `value() -> u32` | none | |
| `set_value(value: u32)` | **admin** | host `Error(Auth, InvalidAction)` if not authorized |
| `set_value_as(caller: Address, value: u32) -> Result<(), AuthGuardError>` | **caller** | `NotAdmin` if caller ≠ admin |
| `transfer_admin(new_admin: Address) -> Result<(), AuthGuardError>` | **admin and new_admin** | `SameAdmin`; host auth error if either signature is missing |

Events: `value_set` (topic: `by`; data: `value`), `admin_transferred`
(topic: `previous`; data: `new_admin`).

## Contract errors

| Code | Variant | Meaning |
|---|---|---|
| 1 | `NotAdmin` | Caller authenticated but is not the admin |
| 2 | `SameAdmin` | `new_admin` equals the current admin |

## Scenarios

| Scenario id | Expected |
|---|---|
| `auth-guard/value/read` | success → `1` |
| `auth-guard/set_value/admin-signed` | success |
| `auth-guard/set_value/missing-auth` | `Error(Auth, InvalidAction)` |
| `auth-guard/set_value/wrong-signer` | `Error(Auth, InvalidAction)` |
| `auth-guard/set_value_as/admin` | success |
| `auth-guard/set_value_as/not-admin` | `Error(Contract, #1)` `NotAdmin` |
| `auth-guard/transfer_admin/both-signed` | success |
| `auth-guard/transfer_admin/partial-auth` | `Error(Auth, InvalidAction)` |
| `auth-guard/transfer_admin/same-admin` | `Error(Contract, #2)` `SameAdmin` |

Note: missing authorization and a wrong signer give the **same** error code.
See [diagnostic-terminology.md](../diagnostic-terminology.md).

## Reproduce locally

```bash
cargo test -p auth-guard                 # unit tests
cargo run -q -p scenario-runner | grep auth-guard
```

## Reproduce on testnet

> ⚠️ **Not yet run end-to-end.** The commands follow stellar-cli v25+ syntax.
> The behaviour described is what we *expect*. Please open an issue with the
> actual output.

```bash
stellar keys generate sorobanlens-admin --network testnet --fund
stellar keys generate sorobanlens-other --network testnet --fund
stellar contract build

stellar contract deploy --wasm target/wasm32v1-none/release/auth_guard.wasm \
  --source-account sorobanlens-admin --network testnet --alias auth_guard \
  -- --admin "$(stellar keys address sorobanlens-admin)" --initial_value 1

# Success: the admin is the source account, so it authorizes the call.
stellar contract invoke --id auth_guard --source-account sorobanlens-admin --network testnet \
  -- set_value --value 42

# Failure: the admin's authorization is missing. The CLI is expected to fail
# while preparing or simulating, because it does not hold the admin's key.
# That is a client-side failure, not an on-chain one.
stellar contract invoke --id auth_guard --source-account sorobanlens-other --network testnet \
  -- set_value --value 42

# Contract-level failure (NotAdmin): the caller signs correctly but is not the admin.
stellar contract invoke --id auth_guard --source-account sorobanlens-other --network testnet \
  -- set_value_as --caller "$(stellar keys address sorobanlens-other)" --value 42
```

Producing an **on-chain** `Error(Auth, InvalidAction)` (as opposed to a
client-side refusal) probably needs a transaction built with deliberately
incomplete authorization entries. That is planned work: see the README
roadmap.
