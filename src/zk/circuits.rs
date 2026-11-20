//! The two Groth16 circuits of the FleetChain protocol.
//!
//! ## State model
//!
//! A fleet is a list of `FLEET_CELLS` *segments* `(x, y, alive)`. The player's
//! state commitment is
//!
//! ```text
//! C = Poseidon(salt, x_1, y_1, a_1, ..., x_M, y_M, a_M)
//! ```
//!
//! * [`SetupCircuit`] proves that `C` opens to a **legal, in-bounds fleet with
//!   every segment alive**. Segments are *derived* from `(origin, direction)`
//!   per ship and the public fleet spec, so ship shape and length are correct
//!   by construction. Overlapping ships are allowed.
//! * [`ShotCircuit`] proves one **state transition**: given a public old
//!   commitment and a public shot `(x, y)`, it reveals only `hit` (a live
//!   segment sat on that cell) and `alive` (some segment is still alive), and
//!   binds a fresh commitment to the updated state in which every segment on
//!   that cell is dead.

use super::gadgets::*;
use super::poseidon::hash_gadget;
use super::{COORD_BITS, FLEET_CELLS, FLEET_SPEC};
use bellman::{Circuit, ConstraintSystem, SynthesisError};
use bls12_381::Scalar;
use ff::Field;

/// One ship's secret placement: the cell with the smallest coordinates and a
/// direction. Segment `t` is at `(x + t, y)` if horizontal, `(x, y + t)` if
/// vertical.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ShipPlacement {
    pub x: u8,
    pub y: u8,
    pub vertical: bool,
}

/// One cell of a ship, as stored in the committed state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Segment {
    pub x: u8,
    pub y: u8,
    pub alive: bool,
}

pub fn b2s(b: bool) -> Scalar {
    if b {
        Scalar::ONE
    } else {
        Scalar::ZERO
    }
}

pub fn u2s(v: u8) -> Scalar {
    Scalar::from(v as u64)
}

/// Expand placements into the flat segment list (all alive).
pub fn derive_segments(placements: &[ShipPlacement]) -> Vec<Segment> {
    let mut out = Vec::with_capacity(FLEET_CELLS);
    for (p, &len) in placements.iter().zip(FLEET_SPEC.iter()) {
        for t in 0..len as u8 {
            let (x, y) = if p.vertical {
                (p.x, p.y.wrapping_add(t))
            } else {
                (p.x.wrapping_add(t), p.y)
            };
            out.push(Segment { x, y, alive: true });
        }
    }
    out
}

/// The committed-state hash input: `[salt, x_1, y_1, a_1, ...]`.
pub fn commitment_inputs(salt: Scalar, segments: &[Segment]) -> Vec<Scalar> {
    let mut v = Vec::with_capacity(1 + 3 * segments.len());
    v.push(salt);
    for s in segments {
        v.extend([u2s(s.x), u2s(s.y), b2s(s.alive)]);
    }
    v
}

/// Native commitment to a fleet state.
pub fn commit(salt: Scalar, segments: &[Segment]) -> Scalar {
    super::poseidon::hash(&commitment_inputs(salt, segments))
}

// ---------------------------------------------------------------- setup ---

/// Witness for [`SetupCircuit`].
#[derive(Clone, Debug)]
pub struct SetupWitness {
    pub salt: Scalar,
    pub placements: Vec<ShipPlacement>,
}

/// Public inputs, in order: `[commitment, board_size]`.
#[derive(Clone, Debug, Default)]
pub struct SetupCircuit {
    pub commitment: Option<Scalar>,
    pub board_size: Option<u8>,
    pub witness: Option<SetupWitness>,
}

impl Circuit<Scalar> for SetupCircuit {
    fn synthesize<CS: ConstraintSystem<Scalar>>(self, cs: &mut CS) -> Result<(), SynthesisError> {
        let commit_in = alloc_input(&mut cs.namespace(|| "commitment"), self.commitment)?;
        let size_in = alloc_input(&mut cs.namespace(|| "board size"), self.board_size.map(u2s))?;
        // W - 1, used for the upper-bound range checks.
        let max_coord = size_in.add_const(-Scalar::ONE);

        let w = self.witness.as_ref();
        if let Some(w) = w {
            if w.placements.len() != FLEET_SPEC.len() {
                return Err(SynthesisError::Unsatisfiable);
            }
        }

        let salt = alloc(&mut cs.namespace(|| "salt"), w.map(|w| w.salt))?;
        let mut inputs = vec![salt];

        for (j, &len) in FLEET_SPEC.iter().enumerate() {
            let mut cs = cs.namespace(|| format!("ship {j}"));
            let p = w.map(|w| w.placements[j]);
            let x0 = alloc(&mut cs.namespace(|| "x0"), p.map(|p| u2s(p.x)))?;
            let y0 = alloc(&mut cs.namespace(|| "y0"), p.map(|p| u2s(p.y)))?;
            let dir = alloc_bit(&mut cs.namespace(|| "dir"), p.map(|p| p.vertical))?;

            for t in 0..len {
                let mut cs = cs.namespace(|| format!("seg {t}"));
                let tt = Scalar::from(t as u64);
                // x_t = x0 + t*(1-dir),   y_t = y0 + t*dir   (both linear)
                let x_t = x0.add_const(tt).sub(&dir.scale(tt));
                let y_t = y0.add(&dir.scale(tt));
                for (name, c) in [("x", &x_t), ("y", &y_t)] {
                    let mut cs = cs.namespace(|| name);
                    // 0 <= c  and  c <= W-1
                    range_check(&mut cs.namespace(|| "lower"), c, COORD_BITS)?;
                    range_check(&mut cs.namespace(|| "upper"), &max_coord.sub(c), COORD_BITS)?;
                }
                inputs.push(x_t);
                inputs.push(y_t);
                inputs.push(Lc::constant(Scalar::ONE)); // alive
            }
        }

        let digest = hash_gadget(&mut cs.namespace(|| "commit"), &inputs)?;
        enforce_equal(
            &mut cs.namespace(|| "opens commitment"),
            &digest,
            &commit_in,
        );
        Ok(())
    }
}

// ----------------------------------------------------------------- shot ---

/// Witness for [`ShotCircuit`].
#[derive(Clone, Debug)]
pub struct ShotWitness {
    pub old_salt: Scalar,
    pub new_salt: Scalar,
    pub segments: Vec<Segment>,
}

/// Public inputs, in order:
/// `[old_commitment, new_commitment, x, y, hit, alive]`.
#[derive(Clone, Debug, Default)]
pub struct ShotCircuit {
    pub old_commitment: Option<Scalar>,
    pub new_commitment: Option<Scalar>,
    pub x: Option<u8>,
    pub y: Option<u8>,
    pub hit: Option<bool>,
    pub alive: Option<bool>,
    pub witness: Option<ShotWitness>,
}

impl Circuit<Scalar> for ShotCircuit {
    fn synthesize<CS: ConstraintSystem<Scalar>>(self, cs: &mut CS) -> Result<(), SynthesisError> {
        let old_in = alloc_input(&mut cs.namespace(|| "old commitment"), self.old_commitment)?;
        let new_in = alloc_input(&mut cs.namespace(|| "new commitment"), self.new_commitment)?;
        let x_in = alloc_input(&mut cs.namespace(|| "shot x"), self.x.map(u2s))?;
        let y_in = alloc_input(&mut cs.namespace(|| "shot y"), self.y.map(u2s))?;
        let hit_in = alloc_input(&mut cs.namespace(|| "hit"), self.hit.map(b2s))?;
        let alive_in = alloc_input(&mut cs.namespace(|| "alive"), self.alive.map(b2s))?;

        let w = self.witness.as_ref();
        if let Some(w) = w {
            if w.segments.len() != FLEET_CELLS {
                return Err(SynthesisError::Unsatisfiable);
            }
        }

        let old_salt = alloc(&mut cs.namespace(|| "old salt"), w.map(|w| w.old_salt))?;
        let new_salt = alloc(&mut cs.namespace(|| "new salt"), w.map(|w| w.new_salt))?;

        let mut old_inputs = vec![old_salt];
        let mut new_inputs = vec![new_salt];
        let mut hits = Lc::zero(); // number of live segments on the target cell
        let mut survivors = Lc::zero(); // number of live segments after the shot

        for i in 0..FLEET_CELLS {
            let mut cs = cs.namespace(|| format!("segment {i}"));
            let s = w.map(|w| w.segments[i]);
            let sx = alloc(&mut cs.namespace(|| "x"), s.map(|s| u2s(s.x)))?;
            let sy = alloc(&mut cs.namespace(|| "y"), s.map(|s| u2s(s.y)))?;
            let sa = alloc(&mut cs.namespace(|| "alive"), s.map(|s| b2s(s.alive)))?;
            enforce_boolean(&mut cs.namespace(|| "alive is boolean"), &sa);

            // match = [x_i == x] * [y_i == y] * alive_i
            let ex = is_zero(&mut cs.namespace(|| "x eq"), &sx.sub(&x_in))?;
            let ey = is_zero(&mut cs.namespace(|| "y eq"), &sy.sub(&y_in))?;
            let at_cell = mul(&mut cs.namespace(|| "at cell"), &ex, &ey)?;
            let m = mul(&mut cs.namespace(|| "live match"), &at_cell, &sa)?;

            // A segment that was hit dies.
            let new_alive = sa.sub(&m);
            hits = hits.add(&m);
            survivors = survivors.add(&new_alive);

            old_inputs.extend([sx.clone(), sy.clone(), sa]);
            new_inputs.extend([sx, sy, new_alive]);
        }

        // hit = [hits != 0],  alive = [survivors != 0]   (both public, boolean)
        enforce_nonzero_flag(&mut cs.namespace(|| "hit flag"), &hits, &hit_in)?;
        enforce_nonzero_flag(&mut cs.namespace(|| "alive flag"), &survivors, &alive_in)?;

        // The witness state opens the old commitment ...
        let old_digest = hash_gadget(&mut cs.namespace(|| "old commit"), &old_inputs)?;
        enforce_equal(&mut cs.namespace(|| "opens old"), &old_digest, &old_in);
        // ... and the updated state (same cells, updated flags) opens the new one.
        let new_digest = hash_gadget(&mut cs.namespace(|| "new commit"), &new_inputs)?;
        enforce_equal(&mut cs.namespace(|| "opens new"), &new_digest, &new_in);
        Ok(())
    }
}
