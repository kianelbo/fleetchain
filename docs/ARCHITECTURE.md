# FleetChain Architecture

## System Overview

FleetChain combines blockchain technology with cryptographic game theory to create a trustless, decentralized Battleship game. The architecture ensures fairness, prevents cheating, and maintains player privacy through cryptographic commitments and zero-knowledge proofs.

## Core Design Principles

1. **Trustless**: No central authority needed; blockchain provides consensus
2. **Privacy-Preserving**: Ship positions hidden via cryptographic commitments
3. **Verifiable**: All actions can be verified without revealing secrets
4. **Fair**: Mining mechanism ensures equitable resource distribution
5. **Tamper-Proof**: Blockchain immutability prevents retroactive changes

## Architecture Layers

```
┌─────────────────────────────────────────────────────────┐
│                    Application Layer                    │
│              (main.rs, examples, CLI)                   │
└─────────────────────────────────────────────────────────┘
                            │
┌─────────────────────────────────────────────────────────┐
│                   Coordinator Layer                     │
│         (GameCoordinator - Orchestration Logic)         │
└─────────────────────────────────────────────────────────┘
                            │
        ┌───────────────────┴───────────────────┐
        │                                       │
┌───────────────────┐                 ┌────────────────────┐
│   Game Logic      │                 │   Blockchain       │
│   Layer           │                 │   Layer            │
│                   │                 │                    │
│ - Grid            │                 │ - Block            │
│ - Ship            │                 │ - Transaction      │
│ - Player          │                 │ - Chain            │
│ - HitReport       │                 │ - Mining           │
└───────────────────┘                 └────────────────────┘
        │                                       │
        └───────────────────┬───────────────────┘
                            │
┌─────────────────────────────────────────────────────────┐
│                  Cryptography Layer                     │
│    (Commitments, ZK Proofs, Hashing, Salt Gen)          │
└─────────────────────────────────────────────────────────┘
```

## Module Breakdown

### 1. Blockchain Module (`blockchain.rs`)

**Purpose**: Provides the underlying distributed ledger for recording game actions.

**Components**:

- **Transaction**: Represents a shot fired by a player
  ```rust
  struct Transaction {
      player_id: String,
      target_x: u8,
      target_y: u8,
      timestamp: i64,
      nonce: u64,
  }
  ```

- **Block**: Container for transactions with proof-of-work
  ```rust
  struct Block {
      index: u64,
      timestamp: i64,
      transactions: Vec<Transaction>,
      previous_hash: String,
      hash: String,
      nonce: u64,
  }
  ```

- **Blockchain**: The chain itself with validation logic
  - Genesis block creation
  - Transaction pooling
  - Mining with configurable difficulty
  - Chain validation

**Key Features**:
- Proof-of-Work mining (configurable difficulty)
- SHA-256 hashing for block integrity
- Immutable transaction history
- Full chain validation

### 2. Game Logic Module (`game.rs`)

**Purpose**: Implements Battleship game rules and state management.

**Components**:

- **Ship**: Represents a ship with positions and hit tracking
  ```rust
  struct Ship {
      id: String,
      positions: Vec<(u8, u8)>,
      hits: Vec<bool>,
  }
  ```

- **Grid**: Shared grid where multiple ships can coexist
  ```rust
  struct Grid {
      size: u8,
      cells: HashMap<(u8, u8), Vec<String>>,
  }
  ```
  - Allows multiple players' ships at same coordinates
  - Tracks which players occupy each cell

- **Player**: Player state including ships, shots, and commitments
  ```rust
  struct Player {
      id: String,
      ships: Vec<Ship>,
      board_commitment: String,
      salt: String,
      shots_available: u32,
      shots_fired: Vec<(u8, u8)>,
  }
  ```

- **HitReport**: Report with ZK proof for verification
  ```rust
  struct HitReport {
      player_id: String,
      shot_x: u8,
      shot_y: u8,
      is_hit: bool,
      proof: Vec<u8>,
  }
  ```

**Key Features**:
- Shared grid system (unique to this implementation)
- Ship placement and hit detection
- Shot tracking and validation
- Player state management

### 3. Cryptography Module (`crypto.rs`)

**Purpose**: Provides cryptographic primitives for privacy and verification.

**Components**:

- **Commitment Scheme**:
  ```rust
  commitment = SHA256(sorted_positions || salt)
  ```
  - Binds player to initial ship placement
  - Prevents retroactive changes
  - Maintains privacy until reveal

- **Salt Generation**:
  - Cryptographically secure random 32-byte salt
  - Prevents rainbow table attacks
  - Adds entropy to commitments

- **Zero-Knowledge Proofs** (Simplified):
  - **HitProof**: Proves hit/miss without revealing other ships
  - `prove_hit()`: Creates proof for a hit
  - `prove_miss()`: Creates proof for a miss
  - `verify_hit()`: Verifies hit claim
  - `verify_miss()`: Verifies miss claim

**Key Features**:
- SHA-256 based commitments
- Secure random salt generation
- ZK proof framework (simplified for prototype)
- Commitment verification

### 4. Coordinator Module (`coordinator.rs`)

**Purpose**: Orchestrates the entire game, bridging blockchain and game logic.

**Components**:

- **GameCoordinator**: Main game controller
  ```rust
  struct GameCoordinator {
      blockchain: Blockchain,
      grid: Grid,
      players: HashMap<String, Player>,
      round: u32,
  }
  ```

**Responsibilities**:
1. Player registration with commitment verification
2. Mining coordination for shot allocation
3. Shot firing and transaction creation
4. Hit report verification with ZK proofs
5. Game state queries and statistics
6. Blockchain validation

**Key Methods**:
- `register_player()`: Register with commitment
- `mine_for_shots()`: Mine to earn shots
- `fire_shot()`: Create shot transaction
- `report_hit()`: Submit hit report with proof
- `verify_blockchain()`: Validate entire chain

## Data Flow

### 1. Player Registration Flow

```
Player → Generate Ships → Create Commitment → Register
                              ↓
                    commitment = SHA256(positions || salt)
                              ↓
                    Store: commitment (public)
                           salt (private)
                           positions (private)
```

### 2. Mining Flow

```
Player → Request Mine → Solve PoW → Create Block → Earn Shots
                          ↓
                    Find nonce where:
                    hash(block) starts with N zeros
                          ↓
                    Add block to chain
                          ↓
                    Award mining_reward shots
```

### 3. Combat Flow

```
Player → Fire Shot → Create Transaction → Add to Pool
                          ↓
                    Transaction {
                        player_id,
                        target_x,
                        target_y,
                        timestamp,
                        nonce
                    }
                          ↓
                    Wait for mining
                          ↓
                    Transaction included in block
```

### 4. Verification Flow

```
Shot Lands → Target Player → Check Hit/Miss → Generate Proof
                                  ↓
                            if HIT:
                              prove_hit(position)
                            else:
                              prove_miss(position)
                                  ↓
                            Submit HitReport
                                  ↓
                            Verify Proof
                                  ↓
                            Update Game State
```

## Security Model

### Threat Model

**Threats Addressed**:
1. **Cheating**: Changing ship positions after game starts
2. **False Reporting**: Lying about hits/misses
3. **Replay Attacks**: Reusing old transactions
4. **Chain Manipulation**: Altering past transactions

**Mitigations**:
1. **Cryptographic Commitments**: Bind players to initial positions
2. **Zero-Knowledge Proofs**: Verify reports without revealing secrets
3. **Timestamps & Nonces**: Prevent replay attacks
4. **Blockchain Validation**: Detect chain tampering

### Trust Assumptions

- **No Trusted Third Party**: System is fully decentralized
- **Honest Majority**: Assumes majority of miners are honest (standard blockchain assumption)
- **Cryptographic Hardness**: SHA-256 collision resistance, discrete log hardness

## Performance Considerations

### Mining Difficulty

- **Low Difficulty (2-3 leading zeros)**: Fast mining, good for demos
- **Medium Difficulty (4-5 leading zeros)**: Balanced gameplay
- **High Difficulty (6+ leading zeros)**: Slow, resource-intensive

### Scalability

**Current Limitations**:
- Single-threaded mining
- In-memory blockchain (no persistence)
- No network layer (local only)

**Future Improvements**:
- Parallel mining with thread pools
- Persistent storage (database)
- P2P networking for multiplayer
- Sharding for large player counts

## Cryptographic Details

### Commitment Scheme

**Construction**:
```
commitment = SHA256(sort(positions) || salt)
```

**Properties**:
- **Binding**: Cannot change positions after commitment
- **Hiding**: Positions not revealed by commitment
- **Deterministic**: Same input always produces same commitment

**Security**:
- Collision resistance: SHA-256 (2^128 security)
- Preimage resistance: Cannot reverse commitment
- Salt prevents rainbow tables

### Zero-Knowledge Proofs (Simplified)

**Current Implementation**:
- Commitment-based proofs (not full ZK-SNARKs)
- Reveals position on hit, hides on miss
- Sufficient for prototype demonstration

**Full ZK-SNARK Implementation** (Future):
- Use bellman + bls12_381 libraries
- Prove "position ∈ ship_positions" without revealing positions
- Prove "position ∉ ship_positions" without revealing positions
- No information leakage

**Circuit Design** (Future):
```
Public Inputs:
  - board_commitment
  - shot_position
  - is_hit (boolean)

Private Inputs:
  - all_ship_positions
  - salt

Constraints:
  1. commitment == SHA256(positions || salt)
  2. is_hit == (shot_position ∈ positions)
```

## Game Theory

### Economic Model

**Mining Rewards**:
- Each mined block → N shots (configurable)
- Creates scarcity and value for shots
- Incentivizes participation in blockchain maintenance

**Strategic Considerations**:
- Players must balance mining vs. shooting
- Shared grid creates complex targeting decisions
- Multiple ships per cell increases hit probability

### Nash Equilibrium

In a multi-player game:
- **Mining**: Necessary to gain shots
- **Shooting**: Necessary to eliminate opponents
- **Optimal Strategy**: Balance mining and combat based on:
  - Number of active players
  - Current shot reserves
  - Blockchain mining difficulty

## Testing Strategy

### Unit Tests

- **Blockchain**: Block creation, mining, validation
- **Game Logic**: Ship hits, grid placement, player state
- **Crypto**: Commitment creation/verification, proof generation
- **Coordinator**: Player registration, mining, shooting

### Integration Tests

- Full game flow from registration to combat
- Multi-player scenarios
- Cheating detection
- Blockchain validation

### Property-Based Tests (Future)

- Commitment binding property
- Blockchain immutability
- Proof soundness and completeness

## Future Enhancements

### Short-Term
1. Full ZK-SNARK implementation
2. Persistent blockchain storage
3. Web-based UI
4. Replay functionality

### Medium-Term
1. P2P networking layer
2. Smart contract-like game rules
3. Tournament mode
4. Different ship types and abilities

### Long-Term
1. Cross-chain compatibility
2. NFT integration for ships
3. Decentralized autonomous organization (DAO) governance
4. Mobile app

## Conclusion

FleetChain demonstrates how blockchain and cryptography can create trustless, verifiable games. The architecture balances simplicity (for educational purposes) with sophistication (demonstrating real cryptographic techniques). While the current implementation is a prototype, it provides a solid foundation for a production-grade decentralized game.
