# FleetChain: Blockchain Battleship

FleetChain is a prototype decentralized blockchain-based Battleship game where:
- Players place their fleets on a **shared grid** (multiple ships can occupy the same cells)
- Ship placements are **hidden** using cryptographic commitments
- Players must **mine** to earn shots
- Shots are recorded as **blockchain transactions**
- Hit reports are verified using **zero-knowledge proofs** to prevent cheating

## Key Features

### Shared Grid System
Unlike traditional Battleship, all players place ships on the same shared grid. Multiple ships from different players can occupy the same cell, creating interesting strategic dynamics.

### Cryptographic Commitments
When registering, players:
1. Place their ships (kept on their own machine)
2. Compute a Poseidon commitment to the fleet and a random salt
3. Submit **only the commitment plus a zero-knowledge proof** that it hides a legal fleet inside the board

The coordinator and all other players never see ship positions or the salt. See [docs/ZK_PROTOCOL.md](docs/ZK_PROTOCOL.md).

### Mining for Shots
Players must mine blocks to earn shots:
- Mining involves solving a proof-of-work puzzle
- Successfully mining a block rewards the player with shot(s)
- This creates a fair resource distribution mechanism

### Blockchain Transactions
Every shot fired is recorded as a blockchain transaction containing:
- Player ID
- Target coordinates (x, y)
- Timestamp
- Nonce

The blockchain provides an immutable, tamper-proof record of all game actions.

### Zero-Knowledge Proofs
Because nobody knows who occupies a cell, every living player (except the shooter) answers every shot with a Groth16 proof:
- The proof shows that the announced **hit/miss** and **alive/sunk** bits are consistent with the player's committed fleet
- It also commits to the updated fleet (hit cells die), so damage persists and old answers cannot be replayed
- Nothing else is revealed: not the cell contents beyond the one bit, not ship shapes, not remaining HP

## Core Components

### Blockchain Module
- **Block**: Contains transactions, hash, previous hash, nonce
- **Transaction**: Represents a shot (player_id, coordinates, timestamp)
- **Blockchain**: Chain of blocks with mining and validation

### Game Module
- **Grid**: Shared grid where ships are placed
- **Ship**: Ship with positions and hit tracking
- **Player**: Client-side view of a fleet (legacy helper; the coordinator no longer stores fleets)
- **ShotRecord / HitReport**: Public shot log entry, and a defender's proof-carrying answer to it

### Crypto Module
- Legacy SHA-256 commitment helpers (no longer used by the game protocol)

### ZK Module
- **Poseidon**: hash over the BLS12-381 scalar field, native + in-circuit
- **Circuits**: `SetupCircuit` (legal fleet) and `ShotCircuit` (hit/miss + state update), Groth16
- **FleetSecret**: player-side secrets; builds registration and shot proofs
- **Verifier / ZkSystem**: key handling and proof verification

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

Once a node is running, you can interact with it via HTTP. See the endpoints [here](docs/NETWORK_GUIDE.md#api-endpoints).
