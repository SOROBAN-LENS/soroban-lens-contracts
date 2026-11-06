# input-validator: invalid arguments

**Source:** [`contracts/input-validator/src/lib.rs`](../../contracts/input-validator/src/lib.rs)
**Category exercised:** `invalid_arguments` (plus one `authorization_failure`)

A minimal limit-order registry. Each validation rule maps to exactly **one**
stable contract error code, so `Error(Contract, #N)` can be turned into a
precise explanation.

## Interface

| Function | Auth | Returns / errors |
|---|---|---|
| `create_order(owner: Address, amount: i128, price: i128, expiry_ledger: u32, memo: String) -> Result<u64, InputValidatorError>` | **owner** | new order id; any validation error |
| `validate_order(amount, price, expiry_ledger, memo) -> Result<(), InputValidatorError>` | none | read-only dry run of the same rules |
| `get_order(id: u64) -> Result<Order, InputValidatorError>` | none | `OrderNotFound` |
| `order_count() -> u64` | none | |

Event: `order_created` (topic: `owner`; data: `id`, `amount`, `price`).

## Validation rules (checked in this order, after `owner.require_auth()`)

| # | Rule | Error |
|---|---|---|
| 1 | `amount > 0` | `AmountNotPositive` (1) |
| 2 | `amount <= MAX_AMOUNT` (10^16, i.e. 1 billion units at 7 decimals) | `AmountTooLarge` (2) |
| 3 | `price > 0` | `PriceNotPositive` (3) |
| 4 | `expiry_ledger > current ledger sequence` | `ExpiryInPast` (4) |
| 5 | `expiry_ledger <= current + MAX_EXPIRY_WINDOW` (535,680 ledgers ≈ 31 days) | `ExpiryTooFar` (5) |
| 6 | `memo.len() <= 32` **bytes** (not characters) | `MemoTooLong` (6) |
| — | `get_order` with an unknown id | `OrderNotFound` (7) |

Arguments of the **wrong type** never reach these rules. The SDK-generated
entry point traps while converting them, which gives `Error(WasmVm, InvalidAction)`
(scenario `input-validator/create_order/wrong-arg-type`). That error is
identical to a panic, so it is a good test of whether the analyzer
overclaims.

## Scenarios

`input-validator/create_order/{valid, amount-zero, amount-too-large,
price-negative, expiry-in-past, expiry-too-far, memo-too-long, missing-auth,
wrong-arg-type}`, `input-validator/get_order/{not-found, existing}`. All
`create_order` scenarios run at ledger sequence 1000.

## Reproduce locally

```bash
cargo test -p input-validator
cargo run -q -p scenario-runner | grep input-validator
```

## Reproduce on testnet

> ⚠️ **Not yet run end-to-end.** Expected behaviour only.

```bash
stellar contract deploy --wasm target/wasm32v1-none/release/input_validator.wasm \
  --source-account sorobanlens-admin --network testnet --alias input_validator

OWNER="$(stellar keys address sorobanlens-admin)"
LATEST=$(stellar ledger latest --network testnet --output json | jq .sequence)  # check `stellar ledger --help` for your CLI version

# Success
stellar contract invoke --id input_validator --source-account sorobanlens-admin --network testnet \
  -- create_order --owner "$OWNER" --amount 100 --price 5 --expiry_ledger $((LATEST + 1000)) --memo "hello"

# Failure: AmountNotPositive (expected: Error(Contract, #1))
stellar contract invoke --id input_validator --source-account sorobanlens-admin --network testnet \
  -- create_order --owner "$OWNER" --amount 0 --price 5 --expiry_ledger $((LATEST + 1000)) --memo "hello"
```

Simulation will catch most of these failures before submission. Use the
CLI's `--send` option (see `stellar contract invoke --help`) to force
submission when you need an on-chain failed transaction hash.
