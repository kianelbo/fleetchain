//! Poseidon hash over the BLS12-381 scalar field: a native implementation and
//! an R1CS gadget that computes exactly the same function.
//!
//! # Parameters
//!
//! * state width `t = 3` (rate 2, capacity 1), S-box `x^5`
//!   (`gcd(5, r-1) = 1` for the BLS12-381 scalar field),
//! * `R_F = 8` full rounds, `R_P = 57` partial rounds: the Poseidon paper's
//!   128-bit-security choice for `t = 3`, `alpha = 5`, a 255-bit prime,
//! * MDS matrix: Cauchy matrix `M[i][j] = 1 / (i + (t + j))`,
//! * round constants: SHA-256 in counter mode under a fixed domain string,
//!   reduced from 512 bits (negligible bias).
//!
//! **These constants are self-generated, not the reference Grain-LFSR
//! instantiation, so this hash is NOT interoperable with other Poseidon
//! libraries.** Have a cryptographer review (or swap in standard constants)
//! before using this for anything of real value.
//!
//! # Sponge
//!
//! Capacity lane 0 is initialised to the input *length*, which makes inputs of
//! different lengths domain-separated; inputs are absorbed two at a time into
//! lanes 1 and 2 (zero-padded); the digest is lane 1.

use super::gadgets::{alloc, Lc};
use bellman::{ConstraintSystem, SynthesisError};
use bls12_381::Scalar;
use ff::Field;
use sha2::{Digest, Sha256};
use std::sync::OnceLock;

pub const T: usize = 3;
pub const RATE: usize = 2;
pub const FULL_ROUNDS: usize = 8;
pub const PARTIAL_ROUNDS: usize = 57;
const TOTAL_ROUNDS: usize = FULL_ROUNDS + PARTIAL_ROUNDS;
const DOMAIN: &[u8] = b"fleetchain/poseidon/bls12-381/t3/alpha5/rf8/rp57/v1";

struct Params {
    rc: Vec<[Scalar; T]>,
    mds: [[Scalar; T]; T],
}

fn params() -> &'static Params {
    static P: OnceLock<Params> = OnceLock::new();
    P.get_or_init(|| {
        let mut rc = Vec::with_capacity(TOTAL_ROUNDS);
        let mut counter = 0u32;
        for _ in 0..TOTAL_ROUNDS {
            let mut row = [Scalar::ZERO; T];
            for lane in row.iter_mut() {
                let mut wide = [0u8; 64];
                for half in 0..2u8 {
                    let mut h = Sha256::new();
                    h.update(DOMAIN);
                    h.update(counter.to_le_bytes());
                    h.update([half]);
                    wide[half as usize * 32..(half as usize + 1) * 32]
                        .copy_from_slice(&h.finalize());
                }
                *lane = Scalar::from_bytes_wide(&wide);
                counter += 1;
            }
            rc.push(row);
        }
        let mut mds = [[Scalar::ZERO; T]; T];
        for (i, row) in mds.iter_mut().enumerate() {
            for (j, m) in row.iter_mut().enumerate() {
                // x_i = i, y_j = T + j  =>  x_i + y_j >= T > 0, so never zero.
                *m = Scalar::from((i + T + j) as u64).invert().unwrap();
            }
        }
        Params { rc, mds }
    })
}

fn is_full_round(r: usize) -> bool {
    r < FULL_ROUNDS / 2 || r >= FULL_ROUNDS / 2 + PARTIAL_ROUNDS
}

fn sbox(x: Scalar) -> Scalar {
    let x2 = x.square();
    x2.square() * x
}

/// The Poseidon permutation on a width-3 state.
pub fn permute(state: &mut [Scalar; T]) {
    let p = params();
    for r in 0..TOTAL_ROUNDS {
        for i in 0..T {
            state[i] += p.rc[r][i];
        }
        if is_full_round(r) {
            for s in state.iter_mut() {
                *s = sbox(*s);
            }
        } else {
            state[0] = sbox(state[0]);
        }
        let old = *state;
        for i in 0..T {
            state[i] = (0..T).map(|j| p.mds[i][j] * old[j]).sum();
        }
    }
}

/// Hash a slice of field elements to one field element.
pub fn hash(inputs: &[Scalar]) -> Scalar {
    let mut state = [
        Scalar::from(inputs.len() as u64),
        Scalar::ZERO,
        Scalar::ZERO,
    ];
    if inputs.is_empty() {
        permute(&mut state);
    }
    for chunk in inputs.chunks(RATE) {
        for (i, v) in chunk.iter().enumerate() {
            state[1 + i] += v;
        }
        permute(&mut state);
    }
    state[1]
}

// ---------------------------------------------------------------- gadget --

/// `x^5` in three constraints; the result is a fresh variable.
fn sbox_gadget<CS: ConstraintSystem<Scalar>>(cs: &mut CS, x: &Lc) -> Result<Lc, SynthesisError> {
    let x2 = alloc(&mut cs.namespace(|| "x2"), x.value().map(|v| v.square()))?;
    let x4 = alloc(&mut cs.namespace(|| "x4"), x2.value().map(|v| v.square()))?;
    let x5 = alloc(
        &mut cs.namespace(|| "x5"),
        x4.value().zip(x.value()).map(|(a, b)| a * b),
    )?;
    let (lx, l2, l4, l5) = (
        x.to_bellman(),
        x2.to_bellman(),
        x4.to_bellman(),
        x5.to_bellman(),
    );
    cs.enforce(|| "x^2", |lc| lc + &lx, |lc| lc + &lx, |lc| lc + &l2);
    cs.enforce(|| "x^4", |lc| lc + &l2, |lc| lc + &l2, |lc| lc + &l4);
    cs.enforce(|| "x^5", |lc| lc + &l4, |lc| lc + &lx, |lc| lc + &l5);
    Ok(x5)
}

fn permute_gadget<CS: ConstraintSystem<Scalar>>(
    cs: &mut CS,
    state: &mut [Lc; T],
) -> Result<(), SynthesisError> {
    let p = params();
    for r in 0..TOTAL_ROUNDS {
        let mut cs = cs.namespace(|| format!("round {r}"));
        for i in 0..T {
            state[i] = state[i].add_const(p.rc[r][i]);
        }
        if is_full_round(r) {
            for i in 0..T {
                state[i] = sbox_gadget(&mut cs.namespace(|| format!("sbox {i}")), &state[i])?;
            }
        } else {
            state[0] = sbox_gadget(&mut cs.namespace(|| "sbox 0"), &state[0])?;
        }
        let old = state.clone();
        for i in 0..T {
            let mut acc = Lc::zero();
            for j in 0..T {
                acc = acc.add(&old[j].scale(p.mds[i][j]));
            }
            state[i] = acc;
        }
    }
    Ok(())
}

/// In-circuit version of [`hash`]. Returns the digest as an `Lc`.
pub fn hash_gadget<CS: ConstraintSystem<Scalar>>(
    cs: &mut CS,
    inputs: &[Lc],
) -> Result<Lc, SynthesisError> {
    let mut state = [
        Lc::constant(Scalar::from(inputs.len() as u64)),
        Lc::zero(),
        Lc::zero(),
    ];
    if inputs.is_empty() {
        permute_gadget(&mut cs.namespace(|| "perm"), &mut state)?;
    }
    for (k, chunk) in inputs.chunks(RATE).enumerate() {
        for (i, v) in chunk.iter().enumerate() {
            state[1 + i] = state[1 + i].add(v);
        }
        permute_gadget(&mut cs.namespace(|| format!("perm {k}")), &mut state)?;
    }
    Ok(state[1].clone())
}
