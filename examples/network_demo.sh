#!/bin/bash

# FleetChain Network Demo
# This script demonstrates how to interact with FleetChain nodes via HTTP API

set -e

BASE_URL="${BASE_URL:-http://localhost:8080}"
# Proving keys created by the node on first start (see --zk-params).
ZK_PARAMS="${ZK_PARAMS:-./data/zk_params.bin}"
# Player secrets (fleet + salt) are kept in plaintext files here and never sent.
SECRETS_DIR="$(mktemp -d)"
RUN_ID="$$"
ALICE="alice_${RUN_ID}"
BOB="bob_${RUN_ID}"

# The zero-knowledge client: builds fleets, proves, and talks to the node.
# Run this script from the repository root.
zk_client() {
    cargo run -q --example zk_client -- --node "${BASE_URL}" --params "${ZK_PARAMS}" "$@"
}

echo "=== FleetChain Network Demo ==="
echo "Make sure you have a node running on port 8080"
echo "Run: cargo run -- --port 8080 --node-id node1"
echo "Run this script from the repository root (it uses the zk_client example)."
echo ""

# Check if node is running
if ! curl -s "${BASE_URL}/api/info" > /dev/null 2>&1; then
    echo "Error: No node running on port 8080"
    echo "Please start a node first: cargo run -- --port 8080 --node-id node1"
    exit 1
fi

echo "✓ Node is running"
echo ""

# Get node info
echo "=== Node Information ==="
curl -s "${BASE_URL}/api/info" | jq '.'
echo ""

# Get initial blockchain
echo "=== Initial Blockchain ==="
curl -s "${BASE_URL}/api/blockchain" | jq '{length: .chain | length, difficulty: .difficulty}'
echo ""

# Register two players. Each fleet is created locally; the node only receives
# a commitment and a zero-knowledge proof that it hides a legal fleet.
echo "=== Registering Players (commitment + ZK proof only) ==="
zk_client register --id "${ALICE}" --secret "${SECRETS_DIR}/alice.json"
zk_client register --id "${BOB}" --secret "${SECRETS_DIR}/bob.json"
echo ""

echo "=== ZK verifying-key fingerprint (all nodes/players must agree) ==="
curl -s "${BASE_URL}/api/zk" | jq -r '.data'
echo ""

# Mine for shots
echo "=== Mining for Shots (Alice) ==="
curl -s -X POST "${BASE_URL}/api/mine" \
  -H "Content-Type: application/json" \
  -d "{\"player_id\": \"${ALICE}\"}" | jq '.'
echo ""

# Fire a shot
echo "=== Alice fires at (5,5) ==="
curl -s -X POST "${BASE_URL}/api/fire" \
  -H "Content-Type: application/json" \
  -d "{\"player_id\": \"${ALICE}\", \"target_x\": 5, \"target_y\": 5}" | jq '.'
echo ""

# Who owes a proof now?
echo "=== Shots Bob owes a proof for ==="
curl -s -X POST "${BASE_URL}/api/pending" \
  -H "Content-Type: application/json" \
  -d "{\"player_id\": \"${BOB}\"}" | jq '.data'
echo ""

# Bob answers with a zero-knowledge proof
echo "=== Bob proves his answer ==="
zk_client respond --id "${BOB}" --secret "${SECRETS_DIR}/bob.json"
echo ""

# Public ledger: only hit/alive bits and commitments
echo "=== Public results ledger (all anyone learns) ==="
curl -s "${BASE_URL}/api/results" | jq '.data[1]'
echo ""

# Mine again to include the shot
echo "=== Mining to Include Shot ==="
curl -s -X POST "${BASE_URL}/api/mine" \
  -H "Content-Type: application/json" \
  -d "{\"player_id\": \"${ALICE}\"}" | jq '.'
echo ""

# Get final stats
echo "=== Final Game Stats ==="
curl -s "${BASE_URL}/api/stats" | jq '.'
echo ""

# Get full blockchain
echo "=== Full Blockchain ==="
curl -s "${BASE_URL}/api/blockchain" | jq '{
  chain_length: .chain | length,
  blocks: [.chain[] | {
    index: .index,
    transactions: .transactions | length,
    hash: .hash[0:16]
  }]
}'
echo ""

echo "(Secret fleets are in ${SECRETS_DIR}; delete them when done.)"
echo "=== Demo Complete ==="
