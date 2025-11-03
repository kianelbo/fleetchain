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
└── main.rs          # Demo application
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

# Run the demo
cargo run

# Run tests
cargo test
```

### Demo Output

The demo showcases:
1. Player registration with cryptographic commitments
2. Mining for shots
3. Firing shots (creating transactions)
4. Mining transactions into blocks
5. Blockchain validation
6. Game statistics

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

## Future Enhancements

- [ ] Implement full ZK-SNARKs using bellman/bls12_381
- [ ] Add network layer for multiplayer
- [ ] Implement smart contract-like game rules
- [ ] Add penalty system for false reports
- [ ] Create web-based UI
- [ ] Add replay functionality from blockchain
- [ ] Implement tournament mode
- [ ] Add different ship types and abilities

## Technical Details

### Dependencies
- `sha2`: SHA-256 hashing for commitments and blockchain
- `serde/serde_json`: Serialization for data structures
- `hex`: Hexadecimal encoding for hashes
- `rand`: Random number generation for salts
- `chrono`: Timestamp management
- `bellman/bls12_381`: ZK-SNARK libraries (for future full implementation)

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
