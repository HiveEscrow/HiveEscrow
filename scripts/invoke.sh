#!/usr/bin/env bash
# Usage: ./scripts/invoke.sh <function> [args...]
# Examples:
#   ./scripts/invoke.sh get_task --task-id 0
#   ./scripts/invoke.sh refund --employer GABC... --task-id 0
set -euo pipefail

NETWORK="${NETWORK:-testnet}"
CONTRACT_ID="${CONTRACT_ID:-$(cat .soroban/contract-id.txt)}"
FN="${1:?Usage: invoke.sh <function> [args...]}"
shift

stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source "$STELLAR_SECRET_KEY" \
  --network "$NETWORK" \
  -- "$FN" "$@"
