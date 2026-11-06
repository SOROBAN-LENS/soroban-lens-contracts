#!/usr/bin/env bash
# Deploys the three SorobanLens fixture contracts to a Stellar network
# (testnet by default) and records their contract IDs.
#
# STATUS: written against stellar-cli v25+ command syntax but NOT yet
# executed end-to-end by the maintainers. See docs/verification.md. Please
# report problems via a GitHub issue.
#
# Prerequisites:
#   - stellar-cli >= 25.2.0
#   - A funded identity:  stellar keys generate "$STELLAR_ADMIN_IDENTITY" --network testnet --fund
#
# Usage:  cp .env.example .env && scripts/deploy-testnet.sh
set -euo pipefail

cd "$(dirname "$0")/.."

if [[ -f .env ]]; then
  # shellcheck disable=SC1091
  set -a; source .env; set +a
fi

NETWORK="${STELLAR_NETWORK:-testnet}"
ADMIN="${STELLAR_ADMIN_IDENTITY:?set STELLAR_ADMIN_IDENTITY (see .env.example)}"
WASM_DIR="target/wasm32v1-none/release"
OUT="deployments/${NETWORK}.local.json"

command -v stellar >/dev/null || { echo "error: stellar-cli not found (https://github.com/stellar/stellar-cli)" >&2; exit 1; }

echo "==> Building contracts"
stellar contract build

ADMIN_ADDR="$(stellar keys address "$ADMIN")"
echo "==> Deploying to '$NETWORK' as $ADMIN ($ADMIN_ADDR)"

deploy() {
  local wasm="$1" alias="$2"; shift 2
  stellar contract deploy \
    --wasm "$WASM_DIR/$wasm" \
    --source-account "$ADMIN" \
    --network "$NETWORK" \
    --alias "$alias" \
    -- "$@"
}

AUTH_GUARD_ID="$(deploy auth_guard.wasm sorobanlens_auth_guard --admin "$ADMIN_ADDR" --initial_value 1)"
INPUT_VALIDATOR_ID="$(deploy input_validator.wasm sorobanlens_input_validator)"
EXECUTION_FAILURE_ID="$(deploy execution_failure.wasm sorobanlens_execution_failure --admin "$ADMIN_ADDR")"

cat > "$OUT" <<JSON
{
  "network": "$NETWORK",
  "admin": "$ADMIN_ADDR",
  "contracts": {
    "auth-guard": "$AUTH_GUARD_ID",
    "input-validator": "$INPUT_VALIDATOR_ID",
    "execution-failure": "$EXECUTION_FAILURE_ID"
  }
}
JSON

echo "==> Done. Contract IDs written to $OUT"
cat "$OUT"
