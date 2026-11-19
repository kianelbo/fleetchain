# Zero-Knowledge Hit/Miss Protocol

FleetChain players never reveal where their ships are. Instead each player
**commits** to a fleet once, and every time a shot is fired at them they publish
a **Groth16 proof** that their answer ("hit"/"miss", "still alive"/"sunk") is
consistent with that commitment. Implementation: `src/zk/`.

* Proof system: Groth16 over BLS12-381 (`bellman` 0.14, `bls12_381` 0.8)
* Hash: Poseidon over the BLS12-381 scalar field (`src/zk/poseidon.rs`)
* Circuits: `src/zk/circuits.rs`
* Player-side secrets: `src/zk/client.rs` (`FleetSecret`)
* Verification: `src/zk/mod.rs` (`Verifier`), used by `GameCoordinator`

## State model

A fleet is a flat list of `M = 10` **segments** (one per ship cell; ship lengths
are the public spec `[4, 3, 2, 1]`). Each segment is `(x, y, alive)`. The
player's state commitment is

```
C = Poseidon(salt, x_1, y_1, a_1, ..., x_M, y_M, a_M)
```

There is no grid and no Merkle tree: the board can be arbitrarily large and any
number of ships may sit on one cell, because the committed state is just a short
list of coordinates.

## Phase 1: registration (`SetupCircuit`)

Public inputs: `C0`, `board_size`. Private: `salt`, and per ship an origin
`(x0, y0)` and direction bit.

The circuit **derives** segment `t` of a ship as `(x0 + t·(1-d), y0 + t·d)`, so
ships are straight and have the right length *by construction* (no separate
shape check is needed), and it range-checks that every derived coordinate lies
in `[0, board_size)`. Every segment starts alive, and the circuit enforces that
`C0` is the Poseidon commitment to exactly that state.

Ships of the same player **may overlap**; so may ships of different players.

The coordinator verifies the proof (with *its own* `board_size`) and stores `C0`
as the player's current commitment.

## Phase 2: answering a shot (`ShotCircuit`)

Every shot is appended to a public, ordered **shot log**. Because nobody knows
who occupies a cell, **every living player except the shooter** owes a proof for
every shot, in order.

Public inputs: `C_old, C_new, x, y, hit, alive`. Private: both salts and the
segment list. The circuit enforces:

1. the private segments open `C_old`;
2. `match_i = [x_i = x] · [y_i = y] · alive_i` (a *live* segment on the target);
3. `hit = [Σ match_i ≠ 0]`, with `hit` constrained boolean;
4. `alive'_i = alive_i − match_i`: **everything on the cell dies**, so damage
   persists and re-shooting a sunk cell is a plain miss;
5. `alive = [Σ alive'_i ≠ 0]`, constrained boolean;
6. `C_new` is the commitment (fresh salt) to the **same coordinates** with the
   updated flags.

The coordinator builds the public inputs **itself**: `C_old` is its stored
commitment for that player, `x, y` come from its shot log. Only `C_new`, `hit`,
`alive` and the proof come from the defender. On success `C_new` replaces `C_old`.

## Why a defender cannot cheat

| Attack | Why it fails |
|---|---|
| Report "miss" on a real hit (or "hit" on a miss) | `hit` is bound to the committed segments by the proof; no witness satisfies a false claim. |
| Claim "still alive" after the last hit / "sunk" early | `alive` is computed in-circuit from the live segments. |
| Move or add ships after seeing shots | Step 6 forces the same coordinates; `C0` was proven legal and is binding. |
| Resurrect a sunk segment | Flags can only go `1 → 0`; step 6 recomputes them. |
| Hit without damage (to be hit repeatedly "for free") | Step 4/6: a hit kills the segment in the committed state. |
| Illegal fleet (off-board, wrong shape or count) | Setup circuit derives and range-checks the fleet; `board_size` is a public input. |
| Replay an old proof | The proof opens a specific `C_old`; once the state moves on it no longer matches. Reports must also be for the defender's *next* owed shot, with a commitment that differs from the current one. |
| Reuse another player's proof / copy their registration | Proofs are bound to `C_old`; duplicate commitments are rejected at registration. |
| Answer a different cell than the one fired | Coordinates come from the coordinator's log, not the report. |
| Stay silent to avoid a hit | Optional deadline: `response_timeout_blocks` forfeits players who leave a shot unanswered too long. **Off by default.** |

## What leaks

Per shot and per defender: one `hit` bit and one `alive` bit (plus a fresh
commitment). Nothing about unhit cells, ship shapes, orientations or remaining
HP. A defender who is hit at a cell has, of course, told everyone there is a
ship there. Public hit/miss patterns can be used for inference, as in
ordinary Battleship.

## Costs (measured, 1 CPU core, optimised build)

| | constraints | proving | proof | 
|---|---|---|---|
| setup | 4,253 | ≈ 0.9 s | 192 B |
| shot response | 7,852 | ≈ 1.2 s | 192 B |

Proving keys ≈ 6 MB; verifying keys ≈ 2.7 KB. Verification is a few
milliseconds.

## Running a game

* `cargo run -- --zk-params ./data/zk_params.bin` creates the key files if
  missing (`zk_params.bin` for players, `zk_params.bin.vk` for nodes). **All
  nodes and players of a game must use the same keys**; compare
  `GET /api/zk` fingerprints.
* `POST /api/register` `{player_id, board_commitment, setup_proof}` (hex).
* `POST /api/pending` `{player_id}` lists shots you owe a proof for.
* `POST /api/respond` submits a `HitReport` (see `src/game.rs`).
* `GET /api/results` returns the shot log and every verified response.
* `cargo run --example zk_proof_demo` walks through the whole protocol.

## Caveats (read before relying on this)

1. **Trusted setup.** Groth16 needs a per-circuit setup whose "toxic waste"
   lets its holder forge proofs. The setup in this repository is single-party
   and for development. A real deployment needs a multi-party ceremony (or a
   transparent system such as a STARK / Halo2).
2. **Poseidon parameters are self-generated** (constants derived from SHA-256,
   Cauchy MDS), not the reference Grain-LFSR parameters, and have not been
   reviewed by a cryptographer. Do not treat the hash as audited.
3. **No authentication.** As in the original project, nothing authenticates
   API callers; `fire_shot` and `mine` act on any `player_id`. (Responding is
   self-authenticating: only the fleet's owner can produce a valid proof.)
4. **Game state is node-local**, exactly as registrations were before: only
   blocks and transactions are gossiped between nodes. Commitments, the shot
   log and responses are not yet replicated or put on-chain.
5. Shots become answerable when recorded by the coordinator, not when mined
   into a block.
6. Coordinates are `u8`; widening the board needs the circuit's `COORD_BITS`
   and the `u8` API types changed together (and new keys).
7. Proving takes about a second per shot; a shot fired at N players costs N
   proofs overall.
