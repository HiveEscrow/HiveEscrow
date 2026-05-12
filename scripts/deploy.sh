#!/usr/bin/env bash
# Usage: ./scripts/deploy.sh [testnet|mainnet]
set -euo pipefail

NETWORK="${1:-testnet}"
WASM="target/wasm32-unknown-unknown/release/hive_escrow.wasm"

echo "▶ Building..."
cargo build --manifest-path contracts/hive-escrow/Cargo.toml \
  --target wasm32-unknown-unknown --release

echo "▶ Optimizing..."
stellar contract optimize --wasm "$WASM"

echo "▶ Deploying to $NETWORK..."
CONTRACT_ID=$(stellar contract deploy \
  --wasm "$WASM" \
  --source "$STELLAR_SECRET_KEY" \
  --network "$NETWORK")

echo "✔ Deployed: $CONTRACT_ID"
echo "$CONTRACT_ID" > .soroban/contract-id.txt
