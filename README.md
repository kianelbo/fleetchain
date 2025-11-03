# FleetChain: Blockchain Battleship

A decentralized Battleship game implementation using blockchain technology and cryptographic proofs in Rust.

## Overview

FleetChain is a prototype blockchain-based Battleship game where:
- Players place their fleets on a **shared grid** where multiple ships can occupy the same cells
- Ship placements are **hidden** using cryptographic commitments
- Players must **mine** to earn shots
- Shots are recorded as **blockchain transactions**
- Hit reports are verified using **zero-knowledge proofs** to prevent cheating

## Key Features

### 🎯 Shared Grid System
Unlike traditional Battleship, all players place ships on the same shared grid. Multiple ships from different players can occupy the same cell, creating interesting strategic dynamics.

### 🔐 Cryptographic Commitments
When registering, players:
1. Place their ships on the grid
2. Generate a cryptographic commitment: `commitment = SHA256(ship_positions || salt)`
3. Submit the commitment hash (without revealing the salt)
4. The salt is kept secret for later verification

This prevents cheating while keeping ship positions hidden.

### ⛏️ Mining for Shots
Players must mine blocks to earn shots:
- Mining involves solving a proof-of-work puzzle
- Successfully mining a block rewards the player with shot(s)
- This creates a fair resource distribution mechanism

### 📜 Blockchain Transactions
Every shot fired is recorded as a blockchain transaction containing:
- Player ID
- Target coordinates (x, y)
- Timestamp
- Nonce

The blockchain provides an immutable, tamper-proof record of all game actions.

### 🔍 Zero-Knowledge Proofs
After each shot, targeted players must report hits/misses with ZK proofs:
- **Hit proof**: Reveals the specific hit position with a commitment
- **Miss proof**: Proves no ship exists at that position without revealing ship locations
- Prevents false reporting while maintaining privacy

## Architecture

```
src/
├── blockchain.rs    # Blockchain implementation (Block, Transaction, Chain)
├── game.rs          # Game logic (Grid, Ship, Player, HitReport)
├── crypto.rs        # Cryptographic functions (commitments, ZK proofs)
├── coordinator.rs   # Game coordinator (orchestrates blockchain + game state)
├── network.rs       # Network node and peer management
├── api.rs           # HTTP API endpoints for node communication
└── main.rs          # Network node application
```

## Core Components

### Blockchain Module
- **Block**: Contains transactions, hash, previous hash, nonce
- **Transaction**: Represents a shot (player_id, coordinates, timestamp)
- **Blockchain**: Chain of blocks with mining and validation

### Game Module
- **Grid**: Shared grid where ships are placed
- **Ship**: Ship with positions and hit tracking
- **Player**: Player state including ships, commitments, and shots
- **HitReport**: Report with ZK proof for verification

### Crypto Module
- **Commitment Scheme**: SHA256-based commitments for ship positions
- **ZK Proofs**: Simplified zero-knowledge proofs for hit/miss verification
- **Salt Generation**: Secure random salt generation

### Coordinator Module
- **GameCoordinator**: Orchestrates the entire game
- Manages player registration, mining, shooting, and verification
- Maintains both blockchain and game state

## Usage

### Build and Run

```bash
# Build the project
cargo build --release

# Run a single node
cargo run -- --port 8080 --node-id node1

# Run with demo mode (includes test game)
cargo run -- --port 8080 --node-id node1 --demo

# Run tests
cargo test
```

### Running a Multi-Node Network

Start multiple nodes and connect them as peers:

```bash
# Terminal 1: Start first node
cargo run -- --port 8080 --node-id node1

# Terminal 2: Start second node and connect to first
cargo run -- --port 8081 --node-id node2 --peers localhost:8080

# Terminal 3: Start third node and connect to network
cargo run -- --port 8082 --node-id node3 --peers localhost:8080,localhost:8081
```

### Command Line Options

```
Options:
  -p, --port <PORT>              Port to run the node on [default: 8080]
  -n, --node-id <NODE_ID>        Node ID (unique identifier) [default: node1]
  -g, --grid-size <GRID_SIZE>    Grid size for battleship [default: 10]
  -d, --difficulty <DIFFICULTY>  Mining difficulty [default: 2]
      --peers <PEERS>            Peer addresses (format: host:port,host:port)
      --demo                     Run in demo mode with test game
  -h, --help                     Print help
  -V, --version                  Print version
```

### HTTP API Endpoints

Once a node is running, you can interact with it via HTTP:

#### Blockchain Endpoints
- `GET /api/blockchain` - Get the entire blockchain
- `POST /api/block` - Receive a new block from peer
- `POST /api/transaction` - Receive a new transaction from peer

#### Game Endpoints
- `POST /api/register` - Register a new player
  ```json
  {
    "player_id": "player1",
    "ships": [...],
    "board_commitment": "abc123...",
    "salt": "xyz789..."
  }
  ```
- `POST /api/fire` - Fire a shot
  ```json
  {
    "player_id": "player1",
    "target_x": 5,
    "target_y": 5
  }
  ```
- `POST /api/mine` - Mine for shots
  ```json
  {
    "player_id": "player1"
  }
  ```

#### Network Endpoints
- `GET /api/peers` - Get all connected peers
- `POST /api/peers` - Add a new peer
  ```json
  {
    "address": "localhost",
    "port": 8081
  }
  ```
- `POST /api/sync` - Synchronize blockchain with all peers

#### Info Endpoints
- `GET /api/info` - Get node information
- `GET /api/stats` - Get game statistics

### Example: Playing via API

```bash
# Register a player on node 1
curl -X POST http://localhost:8080/api/register \
  -H "Content-Type: application/json" \
  -d '{
    "player_id": "alice",
    "ships": [
      {
        "id": "carrier",
        "positions": [[0,0],[0,1],[0,2],[0,3],[0,4]],
        "hits": [false,false,false,false,false]
      }
    ],
    "board_commitment": "...",
    "salt": "..."
  }'

# Mine for shots
curl -X POST http://localhost:8080/api/mine \
  -H "Content-Type: application/json" \
  -d '{"player_id": "alice"}'

# Fire a shot (automatically broadcasts to peers)
curl -X POST http://localhost:8080/api/fire \
  -H "Content-Type: application/json" \
  -d '{"player_id": "alice", "target_x": 5, "target_y": 5}'

# Check node info
curl http://localhost:8080/api/info

# Get blockchain
curl http://localhost:8080/api/blockchain
```

## Game Flow

1. **Registration Phase**
   ```rust
   // Player creates ships
   let ships = vec![Ship::new("carrier", positions)];
   
   // Generate commitment
   let salt = generate_salt();
   let commitment = create_commitment(&positions, &salt);
   
   // Register with game
   game.register_player(player_id, ships, commitment, salt);
   ```

2. **Mining Phase**
   ```rust
   // Mine to earn shots
   let shots_earned = game.mine_for_shots(player_id)?;
   ```

3. **Combat Phase**
   ```rust
   // Fire a shot (creates transaction)
   game.fire_shot(player_id, target_x, target_y)?;
   
   // Mine transactions into blockchain
   game.mine_for_shots(miner_id)?;
   ```

4. **Verification Phase**
   ```rust
   // Generate proof for hit/miss
   let proof = if is_hit {
       HitProof::prove_hit(position, all_positions, salt)
   } else {
       HitProof::prove_miss(position, all_positions, salt)
   };
   
   // Submit report with proof
   let report = HitReport::new(player_id, x, y, is_hit, proof.serialize());
   game.report_hit(report)?;
   ```

## Security Features

- **Tamper-Proof**: Blockchain ensures game history cannot be altered
- **Commitment Scheme**: Ship positions hidden until verification needed
- **ZK Proofs**: Hit/miss reports verified without revealing ship locations
- **Mining**: Fair shot distribution through proof-of-work
- **Validation**: Full blockchain validation ensures integrity

## Network Features

### 🌐 Distributed Blockchain
- Each node maintains its own copy of the blockchain
- Nodes automatically synchronize when connecting to peers
- Longest valid chain wins (consensus mechanism)

### 📡 Gossip Protocol
- New transactions are broadcast to all connected peers
- Newly mined blocks are propagated across the network
- Automatic peer discovery and announcement

### 🔄 Peer-to-Peer Communication
- HTTP-based communication between nodes
- RESTful API for all game actions
- Automatic blockchain synchronization on startup

### 🎮 Multiplayer Support
- Players can join from any node in the network
- Actions on one node are visible to all peers
- Shared game state across the network

## Future Enhancements

- [ ] Implement full ZK-SNARKs using bellman/bls12_381
- [x] Add network layer for multiplayer (✓ Completed)
- [ ] Implement smart contract-like game rules
- [ ] Add penalty system for false reports
- [ ] Create web-based UI
- [ ] Add replay functionality from blockchain
- [ ] Implement tournament mode
- [ ] Add different ship types and abilities
- [ ] Add WebSocket support for real-time updates
- [ ] Implement DHT for better peer discovery
- [ ] Add NAT traversal for public internet play

## Technical Details

### Dependencies
- `sha2`: SHA-256 hashing for commitments and blockchain
- `serde/serde_json`: Serialization for data structures
- `hex`: Hexadecimal encoding for hashes
- `rand`: Random number generation for salts
- `chrono`: Timestamp management
- `bellman/bls12_381`: ZK-SNARK libraries (for future full implementation)
- `tokio`: Async runtime for network operations
- `axum`: Web framework for HTTP API
- `reqwest`: HTTP client for peer communication
- `clap`: Command-line argument parsing

### Mining Difficulty
The mining difficulty determines how many leading zeros are required in block hashes. Higher difficulty = more computation required = fairer distribution.

### Commitment Scheme
```
commitment = SHA256(sorted_positions || salt)
```
Positions are sorted for deterministic hashing. The salt adds randomness to prevent rainbow table attacks.

## Testing

Run the test suite:
```bash
cargo test
```

Tests cover:
- Blockchain creation and validation
- Ship hit detection and sinking
- Grid placement with multiple ships per cell
- Commitment creation and verification
- ZK proof generation and verification
- Player registration and game flow

## License

MIT License - Feel free to use this prototype for learning and experimentation.

## Contributing

This is a prototype implementation. Contributions welcome for:
- Full ZK-SNARK implementation
- Network/multiplayer features
- UI improvements
- Security enhancements
- Performance optimizations

## Disclaimer

This is a prototype for educational purposes. The ZK proof implementation is simplified. For production use, implement proper ZK-SNARKs or ZK-STARKs.
