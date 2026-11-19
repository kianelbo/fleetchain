# FleetChain Architecture

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
│                 (main.rs, examples, CLI)                │
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
│    (Poseidon commitments, Groth16 proofs, SHA-256)       │
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
  - In the coordinator only `size` is meaningful: `cells` stays empty, because
    recording who occupies a cell would reveal every fleet

- **Player**: Client-side view of a fleet (ships, salt). Kept for local use and
  tests; **the coordinator never receives or stores one.** The coordinator's
  record of a player is the public `PlayerState` (commitment, alive flag,
  shots fired, next owed shot).

- **ShotRecord**: entry of the public, ordered shot log (`index`, `shooter`, `x`, `y`, `fired_at_block`)
- **HitReport**: a defender's answer to one shot
  ```rust
  struct HitReport {
      player_id: String,
      shot_index: u64,        // must be the defender's next owed shot
      shot_x: u8,
      shot_y: u8,
      is_hit: bool,
      is_alive: bool,
      new_commitment: String, // hex; commitment to the updated secret state
      proof: Vec<u8>,         // 192-byte Groth16 proof
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

- **Legacy SHA-256 helpers** (`create_commitment`, `verify_commitment`,
  `generate_salt`): kept for compatibility, no longer used by the game protocol.

The zero-knowledge machinery lives in the `zk` module (`src/zk/`):

- **`poseidon.rs`**: Poseidon hash over the BLS12-381 scalar field, native and as an R1CS gadget
- **`circuits.rs`**: `SetupCircuit` (commitment hides a legal in-bounds fleet) and `ShotCircuit` (hit/alive bits + state transition)
- **`client.rs`**: `FleetSecret`, the player's private fleet; creates registration and shot proofs
- **`mod.rs`**: `ZkSystem` (proving + verifying keys), `Verifier` (checks proofs)

See [ZK_PROTOCOL.md](ZK_PROTOCOL.md) for the full protocol, attack table and caveats.

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
1. Player registration: verify the setup proof for a commitment (no fleet is ever received)
2. Mining coordination for shot allocation
3. Shot firing and transaction creation
4. Shot log, per-defender response ordering, Groth16 verification of hit reports, optional response deadlines
5. Game state queries and statistics
6. Blockchain validation

**Key Methods**:
- `register_player(id, commitment_hex, setup_proof)`: Register with commitment + ZK proof
- `pending_shots()`: Shots a player still owes a proof for
- `mine_for_shots()`: Mine to earn shots
- `fire_shot()`: Create shot transaction
- `report_hit()`: Verify a proof-carrying answer and advance the defender's commitment
- `enforce_timeouts()`: Forfeit players past `response_timeout_blocks`
- `verify_blockchain()`: Validate entire chain

## Data Flow

### 1. Player Registration Flow

```
Player → Choose fleet (local) → C0 = Poseidon(salt, fleet) → Prove "C0 hides a legal fleet"
                                        ↓
                    Send to coordinator: C0 + proof  (public)
                    Keep locally:        fleet + salt (private, never sent)
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
Shot appended to public shot log
        ↓
Every living player except the shooter, in shot order:
        ↓
Defender (locally): hit? alive? new state, fresh salt → C_new
        ↓
Prove: "state opening C_old gives these bits and C_new"   (Groth16)
        ↓
Submit HitReport {is_hit, is_alive, new_commitment, proof}
        ↓
Coordinator verifies with public inputs it builds itself
  (C_old from its records, x/y from its shot log)
        ↓
Accept: state := C_new, ledger gains (shot, player, hit, alive)
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

## Cryptographic Details

### Commitment Scheme

```
C = Poseidon(salt, x_1, y_1, alive_1, ..., x_M, y_M, alive_M)
```

- **Binding**: collision resistance of Poseidon; the fleet cannot change after registration
- **Hiding**: a 255-bit random salt, refreshed on every state transition
- The commitment is never opened: all checks happen inside zero-knowledge proofs

### Zero-Knowledge Proofs

Groth16 over BLS12-381 via `bellman`. Two circuits:

- `SetupCircuit` (4,253 constraints): public `[commitment, board_size]`; the fleet is derived
  from per-ship origin/direction and range-checked against the board.
- `ShotCircuit` (7,852 constraints): public `[old_commitment, new_commitment, x, y, hit, alive]`;
  proves the hit/alive bits and that the new commitment is the old state with the hit cells killed.

Full details, including the table of attacks and what is leaked, are in
[ZK_PROTOCOL.md](ZK_PROTOCOL.md). **Caveats**: the shipped Groth16 setup is single-party
(development only) and the Poseidon constants are self-generated and unaudited.

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
1. Replicate registrations, the shot log and responses across nodes (or put them on-chain)
2. Multi-party trusted-setup ceremony (or a transparent proof system), standard Poseidon constants, authentication
3. Persistent blockchain storage
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
