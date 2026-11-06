# execution-failure: runtime failures

**Source:** [`contracts/execution-failure/src/lib.rs`](../../contracts/execution-failure/src/lib.rs)
**Categories exercised:** `contract_execution_failure`, `missing_contract_reference`,
`resource_limit_exceeded` (plus `invalid_arguments` and `authorization_failure`)

Each function triggers one predictable failure. Several come in a
**graceful** form (structured contract error) and an **ungraceful** form
(bare panic), to show how much diagnostic information each one keeps.

## Interface and failure modes

| Function | Failure | Error (host / diagnostic events) | Caller via `try_invoke_contract` |
|---|---|---|---|
| `checked_add(a: u64, b: u64)` | overflow, handled | `Error(Contract, #2)` `Overflow` | same |
| `unchecked_add(a: u64, b: u64)` | overflow **panic** | `Error(WasmVm, InvalidAction)` | `Error(Context, InvalidAction)` |
| `divide(n: i128, d: i128)` | `d == 0` / `MIN / -1` | `#1` `DivisionByZero` / `#2` `Overflow` | same |
| `halt()` | `panic_with_error!` | `Error(Contract, #4)` `Halted` | same |
| `set_entry(key: Symbol, value: u32)` | admin auth missing | `Error(Auth, InvalidAction)` | `Error(Context, InvalidAction)` |
| `get_entry(key: Symbol)` | entry absent | `Error(Contract, #3)` `MissingEntry` | same |
| `call_external(contract: Address, func: Symbol)` | no contract at address | `Error(Storage, MissingValue)` | `Error(Context, InvalidAction)` |
| `call_external(...)` | function does not exist | native: `Error(Context, MissingValue)`; Wasm (source-derived): `Error(WasmVm, MissingValue)` | `Error(Context, InvalidAction)` |
| `hash_chain(iterations: u32)` | CPU budget exhausted | `Error(Budget, ExceededLimit)` | **cannot be caught** |
| `hash_chain(iterations > 1,000,000)` | limit check | `Error(Contract, #5)` `TooManyIterations` | same |

The constructor takes `admin: Address` (used only by `set_entry`).

## Contract errors

| Code | Variant |
|---|---|
| 1 | `DivisionByZero` |
| 2 | `Overflow` |
| 3 | `MissingEntry` |
| 4 | `Halted` |
| 5 | `TooManyIterations` |

## Safety notes

- `unchecked_add` exists *only* to show a bare panic. It is not a pattern to
  copy. Soroban's recommended release profile sets `overflow-checks = true`,
  so the overflow traps instead of silently wrapping.
- `hash_chain` is capped at `MAX_HASH_ITERATIONS`, so the loop is always
  bounded. The network's resource limits stop it well before the cap. The fee
  for the resources actually consumed is still charged on a real network.
- `call_external` will call any contract the invoker names, with no
  arguments. Do not reuse this pattern in production contracts.

## Reproduce locally

```bash
cargo test -p execution-failure
cargo run -q -p scenario-runner | grep execution-failure
```

## Reproduce on testnet

> ⚠️ **Not yet run end-to-end.** Expected behaviour only.

```bash
stellar contract deploy --wasm target/wasm32v1-none/release/execution_failure.wasm \
  --source-account sorobanlens-admin --network testnet --alias execution_failure \
  -- --admin "$(stellar keys address sorobanlens-admin)"

stellar contract invoke --id execution_failure --source-account sorobanlens-admin --network testnet \
  -- divide --numerator 10 --denominator 0           # expected: Error(Contract, #1)

stellar contract invoke --id execution_failure --source-account sorobanlens-admin --network testnet \
  -- unchecked_add --a 18446744073709551615 --b 1    # expected: Error(WasmVm, InvalidAction)

stellar contract invoke --id execution_failure --source-account sorobanlens-admin --network testnet \
  -- hash_chain --iterations 1000000                 # expected: resource limit exceeded during simulation
```
