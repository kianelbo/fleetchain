# FleetChain Game Rules

## Overview

FleetChain is a blockchain-based Battleship game where players compete on a shared grid. Unlike traditional Battleship, multiple players can place ships on the same cells, creating unique strategic dynamics.

## Game Setup

### Grid Configuration
- **Grid Size**: 10x10 (configurable)
- **Shared Grid**: All players place ships on the same grid
- **Cell Occupancy**: Multiple ships from different players can occupy the same cell

### Fleet Composition

Each player must deploy exactly 4 ships:
- **Carrier**: 4 cells (horizontal or vertical)
- **Cruiser**: 3 cells (horizontal or vertical)
- **Submarine**: 2 cells (horizontal or vertical)
- **Destroyer**: 1 cell

**Ship Placement Rules**:
- Ships can be placed horizontally or vertically
- Ships can overlap with other players' ships on the shared grid
- All ship positions must be within grid boundaries

## Registration Phase

### Step 1: Place Ships
Players secretly place their ships on the grid by choosing coordinates for each ship.

Example:
```
Carrier: (0,0), (0,1), (0,2), (0,3)     # 4 cells horizontal
Cruiser: (2,0), (2,1), (2,2)            # 3 cells horizontal
Submarine: (4,0), (4,1)                 # 2 cells horizontal
Destroyer: (6,0)                        # 1 cell
```

### Step 2: Create Commitment
Players create a cryptographic commitment to their ship positions:

1. Collect all ship positions
2. Generate a random salt (32 bytes)
3. Compute: `commitment = SHA256(positions || salt)`
4. Submit commitment to the game (salt remains secret)

**Important**: The commitment binds you to these positions. You cannot change them later!

### Step 3: Register
Submit your registration with:
- Player ID
- Ships (positions)
- Commitment hash
- Salt (kept private, verified later)

The game verifies your commitment matches your ships before accepting registration.

## Mining Phase

### Earning Shots

Players must **mine** to earn shots. Mining involves:

1. Request to mine
2. Solve a proof-of-work puzzle (find a nonce where block hash starts with N zeros)
3. Successfully mine a block
4. Receive mining reward (default: 1 shot **UTXO** per block)

**Mining Difficulty**: Configured at game start (2-6 leading zeros typical)

### Mining Strategy

- **More mining** = More shots but less time attacking
- **Less mining** = Fewer shots but more time for strategy
- Balance is key!

### Defeated Players

**When a player is defeated** (all ships sunk):
- They **cannot mine** for new blocks
- They **cannot earn** shot rewards
- They are eliminated from the game
- Their blockchain transactions remain in history

## Combat Phase

### Firing Shots

To fire a shot:

1. **Check shot availability**: Must have shots from mining
2. **Choose target**: Select coordinates (x, y)
3. **Fire**: Creates a transaction on the blockchain
4. **Wait for mining**: Transaction must be mined into a block

**Transaction Format**:
```json
{
  "player_id": "player1",
  "target_x": 5,
  "target_y": 5,
  "timestamp": 1234567890,
  "nonce": 0
}
```

**Shot Accounting (UTXOs)**:
- Each mined block grants one or more **shot UTXOs** to the miner.
- Each fired shot **consumes exactly one unspent shot UTXO**.
- A player **cannot fire** if they have no unspent shot UTXOs.
- Nodes can expose an API (e.g. `/api/shots`) to query the current unspent shot count for a player.

### Shot Resolution

After a shot is mined into the blockchain:

1. All players at the target coordinates are notified
2. Each affected player must report hit/miss
3. Reports must include zero-knowledge proofs

## Verification Phase

### Hit Reports

When your ship is targeted, you must report:

**For a HIT**:
1. Confirm the hit
2. Generate proof revealing the specific hit position
3. Submit `HitReport` with proof

**For a MISS**:
1. Confirm the miss
2. Generate proof WITHOUT revealing ship positions
3. Submit `HitReport` with proof

### Zero-Knowledge Proofs

**Hit Proof**:
- Reveals the specific position that was hit
- Proves it matches your commitment
- Doesn't reveal other ship positions

**Miss Proof**:
- Proves no ship exists at that position
- Doesn't reveal where your ships actually are
- Verifies against your original commitment

### Verification Process

The game verifies each report:
1. Deserialize the proof
2. Check proof validity against player's commitment
3. Accept or reject the report
4. Update game state if valid

**Cheating Detection**: Invalid proofs are rejected, preventing false reports.

## Victory Conditions

### Ship Sinking

A ship is sunk when all its positions are hit:
- **Carrier**: 4 hits to sink
- **Cruiser**: 3 hits to sink
- **Submarine**: 2 hits to sink
- **Destroyer**: 1 hit to sink

### Player Elimination

A player is **defeated** when ALL their ships are sunk.

**Consequences of Defeat**:
- Cannot mine new blocks
- Cannot earn shot rewards from mining
- Cannot fire new shots (if they have no shots remaining)
- Removed from active player list

### Game Winner

Last player with unsunk ships wins!

## Blockchain Mechanics

### Transaction Pool

Shots are added to a pending transaction pool until mined.

### Block Mining

Miners (players) compete to mine blocks:
- Collect pending transactions
- Solve proof-of-work puzzle
- Add block to chain
- Earn shot rewards

### Chain Validation

The blockchain is continuously validated:
- Each block's hash must be correct
- Each block must reference previous block
- All blocks must meet difficulty requirement

**Tamper Detection**: Any attempt to modify past transactions is detected and rejected.

## Strategic Considerations

### Shared Grid Dynamics

Since multiple ships can occupy the same cell:

**Advantages**:
- Higher hit probability in popular areas
- Can "hide" behind other players' ships
- Unpredictable targeting

**Disadvantages**:
- Your ships might be in high-traffic areas
- Multiple players can be hit by one shot
- Harder to deduce opponent positions

### Resource Management

Balance three activities:
1. **Mining**: Earn shots
2. **Attacking**: Eliminate opponents
3. **Reporting**: Respond to incoming fire

### Timing Strategy

- **Early game**: Focus on mining to build shot reserves
- **Mid game**: Balance mining and attacking
- **Late game**: Use accumulated shots aggressively

### Placement Strategy

**Clustering**:
- Place ships close together
- Easier to defend
- Higher risk if discovered

**Spreading**:
- Distribute ships across grid
- Harder to find
- More vulnerable to random shots

**Overlap Zones**:
- Place ships where others likely are
- Benefit from confusion
- Risk getting hit more often

## Anti-Cheating Mechanisms

### Commitment Scheme

**Prevents**:
- Moving ships after seeing opponent shots
- Changing fleet composition mid-game
- Retroactive strategy changes

**How**: Cryptographic binding to initial positions

### Zero-Knowledge Proofs

**Prevents**:
- False hit claims
- False miss claims
- Revealing opponent positions

**How**: Cryptographic verification without revealing secrets

### Blockchain Immutability

**Prevents**:
- Deleting past shots
- Modifying transaction history
- Denying previous actions

**How**: Cryptographic chain linking with proof-of-work

## Game Flow Summary

```
1. SETUP
   ├─ Place ships
   ├─ Create commitment
   └─ Register

2. ROUND LOOP
   ├─ MINING PHASE
   │  ├─ Players mine for shots
   │  └─ Earn shot rewards
   │
   ├─ COMBAT PHASE
   │  ├─ Players fire shots
   │  └─ Transactions added to pool
   │
   ├─ MINING PHASE
   │  ├─ Mine combat transactions
   │  └─ Shots recorded on blockchain
   │
   └─ VERIFICATION PHASE
      ├─ Targeted players report hits/misses
      ├─ Submit zero-knowledge proofs
      └─ Update game state

3. VICTORY
   └─ Last player standing wins
```

## Example Game Scenario

### Turn 1
- **Player 1** mines → earns 1 shot
- **Player 2** mines → earns 1 shot
- **Player 3** mines → earns 1 shot

### Turn 2
- **Player 1** fires at (5,5) → transaction created
- **Player 2** fires at (0,0) → transaction created
- **Player 3** mines → earns 1 shot, mines combat transactions

### Turn 3
- **Player 2** reports: HIT at (0,0) with proof
- **Player 3** reports: MISS at (5,5) with proof
- Game state updated

### Turn 4
- Players continue mining and shooting
- Eventually, Player 2's carrier is sunk
- Player 2 still has other ships, continues playing

### Game End
- Player 1's last ship is sunk
- Player 3's last ship is sunk
- **Player 2 wins!**

## Tips for New Players

1. **Mine early**: Build up shot reserves
2. **Spread ships**: Don't cluster too much
3. **Track shots**: Remember where you've fired
4. **Verify proofs**: Always check opponent reports
5. **Balance resources**: Don't over-mine or over-shoot
6. **Use blockchain**: Review transaction history for patterns

## Advanced Tactics

### Mining Pools
Coordinate with allies to mine blocks together (future feature).

### Shot Patterns
Use systematic patterns (grid search, spiral, random) to maximize coverage.

### Bluffing
Place ships in unexpected locations to avoid common targeting patterns.

### Endgame
When few players remain, aggressive shooting often beats conservative mining.

## Frequently Asked Questions

**Q: Can I change my ship positions after registering?**
A: No. The cryptographic commitment binds you to your initial positions.

**Q: What happens if I lie about a hit/miss?**
A: Your proof will fail verification and your report will be rejected.

**Q: Can I see other players' ship positions?**
A: No, they're hidden by cryptographic commitments until revealed.

**Q: How many shots can I accumulate?**
A: Unlimited. Mine as much as you want!

**Q: What if two players shoot the same cell?**
A: Both transactions are recorded. Both players spend a shot.

**Q: Can I mine and shoot in the same turn?**
A: Yes! You can perform multiple actions per round.

**Q: Is the blockchain stored permanently?**
A: In this prototype, it's in-memory. Production version would use persistent storage.

## Conclusion

FleetChain combines classic Battleship strategy with blockchain technology and cryptography. Master the balance between mining, combat, and verification to become the ultimate fleet commander!

Good luck, Admiral! ⚓
