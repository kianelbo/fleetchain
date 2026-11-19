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
Your client creates a cryptographic commitment to your fleet:

1. Your ship positions stay **on your machine**
2. A fresh random salt is generated
3. Your client computes `commitment = Poseidon(salt, fleet cells)`

**Important**: The commitment binds you to these positions. You cannot change them later.

### Step 3: Register
Submit your registration with:
- Player ID
- The commitment
- A zero-knowledge proof that the commitment hides a legal fleet (one ship of each
  length 4, 3, 2, 1, each a straight line, all inside the board)

Your positions and salt are **never sent**. The game verifies the proof before accepting
registration, and rejects duplicate IDs and copied commitments.

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

Every fired shot is appended to a public, ordered shot log. Since nobody can see who has ships
on a cell, **every other living player must answer every shot**, in order, with a
zero-knowledge proof. Overlapping ships (yours or other players') on the shot cell are all hit.

## Verification Phase

### Hit Reports

When a shot is fired you work out locally whether it hits a live cell of your fleet, then submit a
`HitReport` containing:

- `is_hit`: whether a live ship cell of yours was on that cell
- `is_alive`: whether any cell of your fleet is still afloat
- `new_commitment`: a commitment to your fleet *after* the shot (hit cells are marked sunk)
- `proof`: a Groth16 proof that all of the above is consistent with your current commitment

A cell that was already hit no longer counts, so shooting it again is reported as a miss.

### Zero-Knowledge Proofs

The proof reveals only the two bits `is_hit` and `is_alive`. It does **not** reveal where
your ships are, how many cells you have left, or anything about cells that were not hit.

### Verification Process

The game verifies each report:
1. The report must answer your next unanswered shot (shots are answered in order)
2. The proof is checked against *your current commitment* and the shot's coordinates, which
   the game takes from its own log
3. If valid, your commitment is replaced by `new_commitment` and the result is added to the
   public results ledger
4. If `is_alive` is false you are out of the game

A deadline can be configured (`--response-timeout-blocks`): a player who leaves a shot
unanswered for more than that many blocks forfeits. It is off by default.

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

**How**: Poseidon commitment to the fleet, proven legal at registration; every later state is a
commitment derived from the previous one inside a proof

### Zero-Knowledge Proofs

**Prevents**:
- False hit claims
- False miss claims
- Revealing opponent positions

**How**: Groth16 proofs bound to the defender's current commitment. A false answer has no valid
proof. See `docs/ZK_PROTOCOL.md` for the attack table and caveats

### Blockchain Immutability

**Prevents**:
- Deleting past shots
- Modifying transaction history
- Denying previous actions

**How**: Cryptographic chain linking with proof-of-work

## Game Flow Summary

```
1. SETUP
   ├─ Place ships (locally)
   ├─ Create commitment + legal-fleet proof
   └─ Register (commitment and proof only)

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
      ├─ Every other living player answers each shot, in order
      ├─ Submit zero-knowledge proofs (hit/alive bits + new commitment)
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

## Frequently Asked Questions

**Q: Can I change my ship positions after registering?**
A: No. The cryptographic commitment binds you to your initial positions.

**Q: What happens if I lie about a hit/miss?**
A: Your proof will fail verification and your report will be rejected.

**Q: Can I see other players' ship positions?**
A: No. They are hidden by cryptographic commitments and are never revealed, not even when the game ends. Each shot only reveals one hit/miss bit per player.

**Q: How many shots can I accumulate?**
A: Unlimited. Mine as much as you want!

**Q: What if two players shoot the same cell?**
A: Both transactions are recorded. Both players spend a shot.

**Q: Can I mine and shoot in the same turn?**
A: Yes! You can perform multiple actions per round.
