//! Player-side secret state. Everything in this file stays on the player's
//! machine; only commitments, flags and proofs are ever sent out.

use super::circuits::{
    commit, derive_segments, Segment, SetupCircuit, SetupWitness, ShipPlacement, ShotCircuit,
    ShotWitness,
};
use super::{scalar_from_hex, scalar_to_hex, ZkSystem, FLEET_CELLS, FLEET_SPEC};
use crate::game::{HitReport, Ship, ShotRecord};
use bls12_381::Scalar;
use ff::Field;
use rand::rngs::OsRng;
use rand::Rng;
use serde::{Deserialize, Serialize};

/// On-disk form of a [`FleetSecret`]. **Contains the secret fleet and salt in
/// plaintext**: whoever can read this file can read the player's board.
#[derive(Serialize, Deserialize)]
struct SecretFile {
    board_size: u8,
    placements: Vec<ShipPlacement>,
    segments: Vec<Segment>,
    salt: String,
    /// Stored only to detect a corrupted or hand-edited file.
    commitment: String,
}

/// A player's secret fleet, its current salt, and the matching commitment.
#[derive(Clone)]
pub struct FleetSecret {
    board_size: u8,
    placements: Vec<ShipPlacement>,
    segments: Vec<Segment>,
    salt: Scalar,
    commitment: Scalar,
}

// Deliberately not derived: a derived `Debug` would print the secret fleet.
impl std::fmt::Debug for FleetSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FleetSecret")
            .field("board_size", &self.board_size)
            .field("commitment", &self.commitment_hex())
            .field("remaining_cells", &self.remaining_cells())
            .field("fleet", &"<redacted>")
            .finish()
    }
}

/// A prepared (proven) response. The fleet only advances to the new state when
/// [`FleetSecret::apply`] is called, so a response that the coordinator never
/// received can be safely re-proven from the same state.
pub struct PendingResponse {
    pub report: HitReport,
    next_segments: Vec<Segment>,
    next_salt: Scalar,
    next_commitment: Scalar,
}

fn is_valid_ship_placement(positions: &[(u8, u8)]) -> bool {
    if positions.is_empty() {
        return false;
    }
    if positions.len() == 1 {
        return true;
    }
    let mut p = positions.to_vec();
    p.sort_unstable();
    let consecutive = |get: fn(&(u8, u8)) -> u8| {
        p.windows(2)
            .all(|w| get(&w[1]) as u16 == get(&w[0]) as u16 + 1)
    };
    if p.iter().all(|&(_, y)| y == p[0].1) {
        return consecutive(|c| c.0);
    }
    if p.iter().all(|&(x, _)| x == p[0].0) {
        return consecutive(|c| c.1);
    }
    false
}

impl FleetSecret {
    /// Validate a fleet and pick a fresh random salt.
    ///
    /// The fleet must be one ship of each length 4, 3, 2, 1, each placed in a
    /// straight line inside the board. Ships of the same player may overlap.
    pub fn from_ships(ships: &[Ship], board_size: u8) -> Result<Self, String> {
        if ships.len() != FLEET_SPEC.len() {
            return Err("Fleet must contain exactly 4 ships".to_string());
        }
        let mut sizes: Vec<usize> = ships.iter().map(|s| s.positions.len()).collect();
        sizes.sort_unstable();
        if sizes != [1, 2, 3, 4] {
            return Err("Fleet must contain: 1 Carrier (4 cells), 1 Cruiser (3 cells), 1 Submarine (2 cells), 1 Destroyer (1 cell)".to_string());
        }
        for ship in ships {
            if !is_valid_ship_placement(&ship.positions) {
                return Err(format!(
                    "Ship '{}' must be placed horizontally or vertically in a continuous line",
                    ship.id
                ));
            }
        }
        // Order by length (4,3,2,1) to line up with FLEET_SPEC.
        let mut ordered: Vec<&Ship> = ships.iter().collect();
        ordered.sort_by_key(|s| std::cmp::Reverse(s.positions.len()));
        let placements = ordered
            .iter()
            .map(|s| {
                let mut p = s.positions.clone();
                p.sort_unstable();
                let vertical = p.len() > 1 && p.iter().all(|&(x, _)| x == p[0].0);
                ShipPlacement {
                    x: p[0].0,
                    y: p[0].1,
                    vertical,
                }
            })
            .collect::<Vec<_>>();
        Self::from_placements(placements, board_size)
    }

    /// Build from explicit placements (ordered 4,3,2,1 cells).
    pub fn from_placements(placements: Vec<ShipPlacement>, board_size: u8) -> Result<Self, String> {
        if placements.len() != FLEET_SPEC.len() {
            return Err("Fleet must contain exactly 4 ships".to_string());
        }
        let segments = derive_segments(&placements);
        debug_assert_eq!(segments.len(), FLEET_CELLS);
        for (p, &len) in placements.iter().zip(FLEET_SPEC.iter()) {
            let end = if p.vertical {
                (p.x as u16, p.y as u16 + len as u16 - 1)
            } else {
                (p.x as u16 + len as u16 - 1, p.y as u16)
            };
            if end.0 >= board_size as u16
                || end.1 >= board_size as u16
                || p.x >= board_size
                || p.y >= board_size
            {
                return Err(format!(
                    "Ship at ({}, {}) extends outside the {}x{} board",
                    p.x, p.y, board_size, board_size
                ));
            }
        }
        let salt = Scalar::random(&mut OsRng);
        let commitment = commit(salt, &segments);
        Ok(FleetSecret {
            board_size,
            placements,
            segments,
            salt,
            commitment,
        })
    }

    /// A random legal fleet (ships may overlap). Handy for demos and bots.
    pub fn random(board_size: u8) -> Result<Self, String> {
        if (board_size as usize) < FLEET_SPEC[0] {
            return Err(format!("Board must be at least {0}x{0}", FLEET_SPEC[0]));
        }
        let mut rng = OsRng;
        let placements = FLEET_SPEC
            .iter()
            .map(|&len| {
                let vertical = rng.gen_bool(0.5);
                let room = board_size - len as u8; // origin may range over 0..=room along the ship's axis
                let (x, y) = if vertical {
                    (rng.gen_range(0..board_size), rng.gen_range(0..=room))
                } else {
                    (rng.gen_range(0..=room), rng.gen_range(0..board_size))
                };
                ShipPlacement { x, y, vertical }
            })
            .collect();
        Self::from_placements(placements, board_size)
    }

    /// Serialise the secret (plaintext!) so a player can resume later.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(&SecretFile {
            board_size: self.board_size,
            placements: self.placements.clone(),
            segments: self.segments.clone(),
            salt: scalar_to_hex(&self.salt),
            commitment: self.commitment_hex(),
        })
        .expect("secret serialises")
    }

    /// Restore from [`Self::to_json`], re-checking that everything is consistent.
    pub fn from_json(json: &str) -> Result<Self, String> {
        let f: SecretFile =
            serde_json::from_str(json).map_err(|e| format!("Bad secret file: {e}"))?;
        // Re-derive the fleet from the placements and check bounds.
        let fresh = Self::from_placements(f.placements.clone(), f.board_size)?;
        let layout = derive_segments(&f.placements);
        if f.segments.len() != FLEET_CELLS
            || f.segments
                .iter()
                .zip(&layout)
                .any(|(a, b)| (a.x, a.y) != (b.x, b.y))
        {
            return Err(
                "Secret file is inconsistent: segments do not match placements".to_string(),
            );
        }
        let salt = scalar_from_hex(&f.salt)?;
        let commitment = commit(salt, &f.segments);
        if scalar_to_hex(&commitment) != f.commitment {
            return Err("Secret file is corrupted: commitment does not match".to_string());
        }
        Ok(FleetSecret {
            board_size: f.board_size,
            placements: fresh.placements,
            segments: f.segments,
            salt,
            commitment,
        })
    }

    pub fn board_size(&self) -> u8 {
        self.board_size
    }

    pub fn commitment(&self) -> Scalar {
        self.commitment
    }

    pub fn commitment_hex(&self) -> String {
        scalar_to_hex(&self.commitment)
    }

    /// Cells of this fleet that are still afloat (private knowledge).
    pub fn remaining_cells(&self) -> usize {
        self.segments.iter().filter(|s| s.alive).count()
    }

    pub fn is_defeated(&self) -> bool {
        self.remaining_cells() == 0
    }

    /// Prove that [`Self::commitment`] opens to a legal in-bounds fleet.
    pub fn prove_setup(&self, sys: &ZkSystem) -> Result<Vec<u8>, String> {
        sys.prove_setup(SetupCircuit {
            commitment: Some(self.commitment),
            board_size: Some(self.board_size),
            witness: Some(SetupWitness {
                salt: self.salt,
                placements: self.placements.clone(),
            }),
        })
    }

    /// Answer a shot: work out hit/alive locally and prove it against the
    /// current commitment. Does not change `self`; call [`Self::apply`] once
    /// the coordinator accepted the response.
    pub fn respond(
        &self,
        sys: &ZkSystem,
        player_id: &str,
        shot: &ShotRecord,
    ) -> Result<PendingResponse, String> {
        let mut next = self.segments.clone();
        let mut hit = false;
        for s in next.iter_mut() {
            if s.alive && s.x == shot.x && s.y == shot.y {
                s.alive = false;
                hit = true;
            }
        }
        let alive = next.iter().any(|s| s.alive);
        let next_salt = Scalar::random(&mut OsRng);
        let next_commitment = commit(next_salt, &next);

        let proof = sys.prove_shot(ShotCircuit {
            old_commitment: Some(self.commitment),
            new_commitment: Some(next_commitment),
            x: Some(shot.x),
            y: Some(shot.y),
            hit: Some(hit),
            alive: Some(alive),
            witness: Some(ShotWitness {
                old_salt: self.salt,
                new_salt: next_salt,
                segments: self.segments.clone(),
            }),
        })?;

        Ok(PendingResponse {
            report: HitReport {
                player_id: player_id.to_string(),
                shot_index: shot.index,
                shot_x: shot.x,
                shot_y: shot.y,
                is_hit: hit,
                is_alive: alive,
                new_commitment: scalar_to_hex(&next_commitment),
                proof,
            },
            next_segments: next,
            next_salt,
            next_commitment,
        })
    }

    /// Advance to the state produced by an accepted response.
    pub fn apply(&mut self, pending: PendingResponse) {
        self.segments = pending.next_segments;
        self.salt = pending.next_salt;
        self.commitment = pending.next_commitment;
    }
}
