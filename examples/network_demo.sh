#!/bin/bash

# FleetChain Network Demo
# This script demonstrates how to interact with FleetChain nodes via HTTP API

set -e

BASE_URL="http://localhost:8080"

echo "=== FleetChain Network Demo ==="
echo "Make sure you have a node running on port 8080"
echo "Run: cargo run -- --port 8080 --node-id node1"
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

# Register a player
echo "=== Registering Player 1 ==="
PLAYER1_SALT=$(openssl rand -hex 32)
# For demo purposes, we'll use a simple commitment (in production, calculate properly)
PLAYER1_COMMITMENT=$(echo -n "player1_ships_${PLAYER1_SALT}" | shasum -a 256 | cut -d' ' -f1)

curl -s -X POST "${BASE_URL}/api/register" \
  -H "Content-Type: application/json" \
  -d "{
    \"player_id\": \"alice\",
    \"ships\": [
      {
        \"id\": \"carrier\",
        \"positions\": [[0,0],[0,1],[0,2],[0,3],[0,4]],
        \"hits\": [false,false,false,false,false]
      },
      {
        \"id\": \"battleship\",
        \"positions\": [[2,0],[2,1],[2,2],[2,3]],
        \"hits\": [false,false,false,false]
      }
    ],
    \"board_commitment\": \"${PLAYER1_COMMITMENT}\",
    \"salt\": \"${PLAYER1_SALT}\"
  }" | jq '.'
echo ""

# Mine for shots
echo "=== Mining for Shots (Player 1) ==="
curl -s -X POST "${BASE_URL}/api/mine" \
  -H "Content-Type: application/json" \
  -d '{"player_id": "alice"}' | jq '.'
echo ""

# Check blockchain after mining
echo "=== Blockchain After Mining ==="
curl -s "${BASE_URL}/api/blockchain" | jq '{length: .chain | length, pending: .pending_transactions | length}'
echo ""

# Fire a shot
echo "=== Firing Shot at (5,5) ==="
curl -s -X POST "${BASE_URL}/api/fire" \
  -H "Content-Type: application/json" \
  -d '{"player_id": "alice", "target_x": 5, "target_y": 5}' | jq '.'
echo ""

# Check pending transactions
echo "=== Pending Transactions ==="
curl -s "${BASE_URL}/api/blockchain" | jq '.pending_transactions'
echo ""

# Mine again to include the shot
echo "=== Mining to Include Shot ==="
curl -s -X POST "${BASE_URL}/api/mine" \
  -H "Content-Type: application/json" \
  -d '{"player_id": "alice"}' | jq '.'
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

echo "=== Demo Complete ==="
