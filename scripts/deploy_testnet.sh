#!/usr/bin/env bash
# Build, deploy and initialize the ticket-registry contract on Stellar testnet.
#
# Usage: ./scripts/deploy_testnet.sh [identity-name]
#
# Requires the Stellar CLI: https://developers.stellar.org/docs/tools/cli/install-cli
set -euo pipefail

IDENTITY="${1:-tickie-deployer}"
NETWORK="testnet"
WASM="target/wasm32v1-none/release/ticket_registry.wasm"

cd "$(dirname "$0")/.."

if ! command -v stellar >/dev/null 2>&1; then
  echo "error: stellar CLI not found. Install it first:" >&2
  echo "  brew install stellar-cli   # or: cargo install --locked stellar-cli" >&2
  exit 1
fi

# Create + fund the deployer identity if it doesn't exist yet (friendbot).
if ! stellar keys address "$IDENTITY" >/dev/null 2>&1; then
  echo "==> Creating and funding testnet identity '$IDENTITY'"
  stellar keys generate "$IDENTITY" --network "$NETWORK" --fund
fi
ADMIN="$(stellar keys address "$IDENTITY")"
echo "==> Deployer/admin address: $ADMIN"

echo "==> Building contract"
stellar contract build

echo "==> Deploying to $NETWORK"
CONTRACT_ID="$(stellar contract deploy \
  --wasm "$WASM" \
  --source-account "$IDENTITY" \
  --network "$NETWORK" \
  -- \
  --admin "$ADMIN")"

echo ""
echo "Deployed ticket-registry:"
echo "  contract id : $CONTRACT_ID"
echo "  admin       : $ADMIN"
echo "  explorer    : https://stellar.expert/explorer/testnet/contract/$CONTRACT_ID"
