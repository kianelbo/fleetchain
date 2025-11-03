# FleetChain Network Guide

## Overview

FleetChain has been transformed from a demo application into a fully distributed blockchain network. Nodes can now communicate with each other via HTTP, synchronize their blockchains, and play the game in a peer-to-peer fashion.

## Architecture

### Network Components

1. **NetworkNode** (`src/network.rs`)
   - Manages peer connections
   - Handles blockchain synchronization
   - Broadcasts transactions and blocks
   - Maintains node state

2. **HTTP API** (`src/api.rs`)
   - RESTful endpoints for all game actions
   - Peer communication interface
   - Blockchain query endpoints
   - Node information endpoints

3. **Gossip Protocol**
   - New transactions are broadcast to all peers
   - Newly mined blocks propagate across the network
   - Automatic peer discovery and announcement

## Running Nodes

### Single Node

```bash
cargo run -- --port 8080 --node-id node1
```

### Multi-Node Network

**Terminal 1 - First Node:**
```bash
cargo run -- --port 8080 --node-id node1
```

**Terminal 2 - Second Node (connects to first):**
```bash
cargo run -- --port 8081 --node-id node2 --peers localhost:8080
```

**Terminal 3 - Third Node (connects to network):**
```bash
cargo run -- --port 8082 --node-id node3 --peers localhost:8080,localhost:8081
```

### Command Line Options

| Option | Description | Default |
|--------|-------------|---------|
| `-p, --port` | Port to run the node on | 8080 |
| `-n, --node-id` | Unique node identifier | node1 |
| `-g, --grid-size` | Battleship grid size | 10 |
| `-d, --difficulty` | Mining difficulty (leading zeros) | 2 |
| `--peers` | Comma-separated peer addresses | none |
| `--demo` | Run with demo game | false |

## API Endpoints

### Blockchain Endpoints

#### GET /api/blockchain
Get the entire blockchain.

**Response:**
```json
{
  "chain": [...],
  "difficulty": 2,
  "pending_transactions": [...],
  "mining_reward": 1
}
```

#### POST /api/block
Receive a new block from a peer (used internally by gossip protocol).

**Request:**
```json
{
  "index": 1,
  "timestamp": 1234567890,
  "transactions": [...],
  "previous_hash": "abc123...",
  "hash": "def456...",
  "nonce": 12345
}
```

#### POST /api/transaction
Receive a new transaction from a peer (used internally by gossip protocol).

**Request:**
```json
{
  "player_id": "alice",
  "target_x": 5,
  "target_y": 5,
  "timestamp": 1234567890,
  "nonce": 0
}
```

### Game Endpoints

#### POST /api/register
Register a new player with their fleet.

**Request:**
```json
{
  "player_id": "alice",
  "ships": [
    {
      "id": "carrier",
      "positions": [[0,0],[0,1],[0,2],[0,3],[0,4]],
      "hits": [false,false,false,false,false]
    }
  ],
  "board_commitment": "abc123...",
  "salt": "xyz789..."
}
```

**Response:**
```json
{
  "success": true,
  "data": "Player alice registered",
  "error": null
}
```

#### POST /api/fire
Fire a shot at coordinates. Automatically broadcasts to all peers.

**Request:**
```json
{
  "player_id": "alice",
  "target_x": 5,
  "target_y": 5
}
```

**Response:**
```json
{
  "success": true,
  "data": "Shot fired and broadcasted",
  "error": null
}
```

#### POST /api/mine
Mine pending transactions to earn shots. Broadcasts new block to peers.

**Request:**
```json
{
  "player_id": "alice"
}
```

**Response:**
```json
{
  "success": true,
  "data": 1,
  "error": null
}
```

### Network Endpoints

#### GET /api/peers
Get all connected peers.

**Response:**
```json
[
  {
    "address": "localhost",
    "port": 8081
  },
  {
    "address": "localhost",
    "port": 8082
  }
]
```

#### POST /api/peers
Add a new peer to the network.

**Request:**
```json
{
  "address": "localhost",
  "port": 8083
}
```

#### POST /api/sync
Manually trigger blockchain synchronization with all peers.

**Response:**
```json
{
  "success": true,
  "data": "Blockchain synchronized",
  "error": null
}
```

### Info Endpoints

#### GET /api/info
Get comprehensive node information.

**Response:**
```json
{
  "node_id": "node1",
  "port": 8080,
  "peers_count": 2,
  "blockchain_info": {
    "length": 5,
    "difficulty": 2,
    "pending_transactions": 1,
    "is_valid": true
  }
}
```

#### GET /api/stats
Get current game statistics.

**Response:**
```json
{
  "round": 0,
  "total_players": 2,
  "active_players": 2,
  "total_shots": 5,
  "blockchain_length": 3
}
```

## Network Behavior

### Peer Discovery

1. When a node starts with `--peers`, it connects to specified peers
2. The node announces itself to each peer via POST /api/peers
3. Peers add the new node to their peer list
4. The network forms a mesh topology

### Blockchain Synchronization

1. On startup, nodes sync with all peers via GET /api/blockchain
2. If a peer has a longer valid chain, the node adopts it
3. Longest valid chain wins (simple consensus)
4. Automatic sync ensures network consistency

### Transaction Propagation

1. Player fires a shot via POST /api/fire
2. Transaction is added to local pending pool
3. Transaction is broadcast to all peers via POST /api/transaction
4. Peers add the transaction to their pending pools
5. Any node can mine the transaction into a block

### Block Propagation

1. Node mines pending transactions via POST /api/mine
2. New block is added to local chain
3. Block is broadcast to all peers via POST /api/block
4. Peers validate and add the block to their chains
5. Blockchain stays synchronized across the network

## Example Workflows

### Complete Game Flow

```bash
# Terminal 1: Start node 1
cargo run -- --port 8080 --node-id node1

# Terminal 2: Start node 2 and connect
cargo run -- --port 8081 --node-id node2 --peers localhost:8080

# Terminal 3: Interact with the network
# Register player on node 1
curl -X POST http://localhost:8080/api/register \
  -H "Content-Type: application/json" \
  -d '{"player_id": "alice", "ships": [...], "board_commitment": "...", "salt": "..."}'

# Mine for shots on node 1
curl -X POST http://localhost:8080/api/mine \
  -H "Content-Type: application/json" \
  -d '{"player_id": "alice"}'

# Fire shot from node 1 (broadcasts to node 2)
curl -X POST http://localhost:8080/api/fire \
  -H "Content-Type: application/json" \
  -d '{"player_id": "alice", "target_x": 5, "target_y": 5}'

# Check blockchain on node 2 (should see the transaction)
curl http://localhost:8081/api/blockchain

# Mine on node 2 (includes alice's shot)
curl -X POST http://localhost:8081/api/mine \
  -H "Content-Type: application/json" \
  -d '{"player_id": "bob"}'

# Both nodes now have the same blockchain
curl http://localhost:8080/api/blockchain
curl http://localhost:8081/api/blockchain
```

### Testing Scripts

Two helper scripts are provided:

1. **test_network.sh** - Basic network functionality test
2. **examples/network_demo.sh** - Complete game flow demonstration

Run them with:
```bash
./test_network.sh
./examples/network_demo.sh
```

## Network Security

### Current Implementation

- **HTTP-based**: Simple, unencrypted communication
- **No authentication**: Any node can join the network
- **Trust-based**: Nodes trust peer-provided data after validation

### Blockchain Validation

All blocks are validated before acceptance:
- Correct index (sequential)
- Valid previous hash (links to existing chain)
- Valid proof-of-work (meets difficulty requirement)
- Valid transaction format

### Future Enhancements

- [ ] HTTPS/TLS for encrypted communication
- [ ] Node authentication and authorization
- [ ] Byzantine fault tolerance
- [ ] Sybil attack protection
- [ ] DDoS mitigation
- [ ] Peer reputation system

## Troubleshooting

### Port Already in Use
```
Error: Address already in use
```
**Solution:** Use a different port with `--port XXXX`

### Cannot Connect to Peer
```
Failed to announce to peer: connection refused
```
**Solution:** Ensure the peer node is running and accessible

### Blockchain Sync Failed
```
Failed to sync with peer: timeout
```
**Solution:** Check network connectivity and peer availability

### Invalid Block Received
```
Invalid previous hash
```
**Solution:** This is normal - the node rejects invalid blocks automatically

## Performance Considerations

### Mining Difficulty

Higher difficulty = slower mining = more CPU usage
- Development: difficulty 2 (fast)
- Testing: difficulty 3-4 (moderate)
- Production: difficulty 5+ (secure)

### Network Latency

- Local network: <10ms
- Internet: 50-200ms
- Blockchain sync time increases with chain length

### Scalability

Current implementation supports:
- ~10-20 nodes efficiently
- Hundreds of transactions per block
- Thousands of blocks in the chain

For larger networks, consider:
- WebSocket for real-time updates
- DHT for peer discovery
- Sharding for scalability

## Conclusion

FleetChain is now a fully functional distributed blockchain network. Nodes can join, play the game, and maintain consensus through the longest valid chain rule. The HTTP API provides a simple interface for all operations, and the gossip protocol ensures network-wide consistency.
