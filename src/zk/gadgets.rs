//! Small R1CS gadget toolkit used by the FleetChain circuits.
//!
//! The central type is [`Lc`], a *merged* linear combination that also tracks
//! its witness value. bellman's own `LinearCombination` does not merge
//! duplicate variables, which makes Poseidon's partial rounds blow up
//! exponentially, so we keep our own sparse representation and only convert to
//! bellman's type at the moment a constraint is enforced.

use bellman::gadgets::boolean::AllocatedBit;
use bellman::{ConstraintSystem, Index, LinearCombination, SynthesisError, Variable};
use bls12_381::Scalar;
use ff::{Field, PrimeField};

/// The constant-one variable of every bellman constraint system.
fn one() -> Variable {
    Variable::new_unchecked(Index::Input(0))
}

/// A linear combination of circuit variables together with its (optional)
/// witness value. `val` is `None` while generating parameters.
#[derive(Clone)]
pub struct Lc {
    terms: Vec<(Variable, Scalar)>,
    val: Option<Scalar>,
}

impl Lc {
    pub fn zero() -> Self {
        Lc { terms: vec![], val: Some(Scalar::ZERO) }
    }

    pub fn constant(c: Scalar) -> Self {
        let mut lc = Lc::zero();
        lc.add_term(one(), c);
        lc.val = Some(c);
        lc
    }

    pub fn var(v: Variable, val: Option<Scalar>) -> Self {
        Lc { terms: vec![(v, Scalar::ONE)], val }
    }

    pub fn value(&self) -> Option<Scalar> {
        self.val
    }

    fn add_term(&mut self, v: Variable, c: Scalar) {
        if bool::from(c.is_zero()) {
            return;
        }
        let idx = v.get_unchecked();
        if let Some(pos) = self.terms.iter().position(|(w, _)| w.get_unchecked() == idx) {
            self.terms[pos].1 += c;
            if bool::from(self.terms[pos].1.is_zero()) {
                self.terms.swap_remove(pos);
            }
        } else {
            self.terms.push((v, c));
        }
    }

    pub fn scale(&self, k: Scalar) -> Lc {
        let mut out = Lc::zero();
        for (v, c) in &self.terms {
            out.add_term(*v, *c * k);
        }
        out.val = self.val.map(|x| x * k);
        out
    }

    pub fn add(&self, o: &Lc) -> Lc {
        let mut out = self.clone();
        for (v, c) in &o.terms {
            out.add_term(*v, *c);
        }
        out.val = self.val.zip(o.val).map(|(a, b)| a + b);
        out
    }

    pub fn sub(&self, o: &Lc) -> Lc {
        self.add(&o.scale(-Scalar::ONE))
    }

    pub fn add_const(&self, c: Scalar) -> Lc {
        let mut out = self.clone();
        out.add_term(one(), c);
        out.val = self.val.map(|x| x + c);
        out
    }

    pub fn to_bellman(&self) -> LinearCombination<Scalar> {
        self.terms
            .iter()
            .fold(LinearCombination::zero(), |lc, (v, c)| lc + (*c, *v))
    }
}

/// Allocate a private variable holding `val`.
pub fn alloc<CS: ConstraintSystem<Scalar>>(
    cs: &mut CS,
    val: Option<Scalar>,
) -> Result<Lc, SynthesisError> {
    let v = cs.alloc(|| "var", || val.ok_or(SynthesisError::AssignmentMissing))?;
    Ok(Lc::var(v, val))
}

/// Allocate a public input holding `val`.
pub fn alloc_input<CS: ConstraintSystem<Scalar>>(
    cs: &mut CS,
    val: Option<Scalar>,
) -> Result<Lc, SynthesisError> {
    let v = cs.alloc_input(|| "input", || val.ok_or(SynthesisError::AssignmentMissing))?;
    Ok(Lc::var(v, val))
}

/// Enforce `a == b` (one constraint).
pub fn enforce_equal<CS: ConstraintSystem<Scalar>>(cs: &mut CS, a: &Lc, b: &Lc) {
    let (a, b) = (a.to_bellman(), b.to_bellman());
    cs.enforce(|| "equal", |lc| lc + &a, |lc| lc + one(), |lc| lc + &b);
}

/// Enforce `x * (x - 1) == 0`.
pub fn enforce_boolean<CS: ConstraintSystem<Scalar>>(cs: &mut CS, x: &Lc) {
    let a = x.to_bellman();
    let b = x.add_const(-Scalar::ONE).to_bellman();
    cs.enforce(|| "boolean", |lc| lc + &a, |lc| lc + &b, |lc| lc);
}

/// Allocate a fresh variable constrained to `a * b`.
pub fn mul<CS: ConstraintSystem<Scalar>>(cs: &mut CS, a: &Lc, b: &Lc) -> Result<Lc, SynthesisError> {
    let val = a.val.zip(b.val).map(|(x, y)| x * y);
    let out = alloc(cs, val)?;
    let (la, lb, lo) = (a.to_bellman(), b.to_bellman(), out.to_bellman());
    cs.enforce(|| "mul", |lc| lc + &la, |lc| lc + &lb, |lc| lc + &lo);
    Ok(out)
}

/// Returns a boolean-valued `Lc` that equals 1 iff `v == 0`. Two constraints.
///
/// `v * inv = 1 - out` and `v * out = 0`:
///   * `v != 0` forces `out = 0` (second), then `inv = 1/v` satisfies the first;
///   * `v == 0` forces `out = 1` (first); the second is trivially satisfied.
pub fn is_zero<CS: ConstraintSystem<Scalar>>(cs: &mut CS, v: &Lc) -> Result<Lc, SynthesisError> {
    let inv = alloc(
        &mut cs.namespace(|| "inv"),
        v.val.map(|x| Option::<Scalar>::from(x.invert()).unwrap_or(Scalar::ZERO)),
    )?;
    let out = alloc(
        &mut cs.namespace(|| "out"),
        v.val.map(|x| if bool::from(x.is_zero()) { Scalar::ONE } else { Scalar::ZERO }),
    )?;
    let (lv, li, lo) = (v.to_bellman(), inv.to_bellman(), out.to_bellman());
    cs.enforce(
        || "v*inv = 1-out",
        |lc| lc + &lv,
        |lc| lc + &li,
        |lc| lc + one() - &lo,
    );
    cs.enforce(|| "v*out = 0", |lc| lc + &lv, |lc| lc + &lo, |lc| lc);
    Ok(out)
}

/// Enforce that `flag` is a boolean equal to `[v != 0]`. Two constraints.
///
/// `flag` must be boolean, and `v * inv = flag` (so `v = 0` forces `flag = 0`
/// while `v != 0` forces `flag = 1` with `inv = 1/v`).
pub fn enforce_nonzero_flag<CS: ConstraintSystem<Scalar>>(
    cs: &mut CS,
    v: &Lc,
    flag: &Lc,
) -> Result<(), SynthesisError> {
    enforce_boolean(cs, flag);
    let inv = alloc(cs, v.val.map(|x| Option::<Scalar>::from(x.invert()).unwrap_or(Scalar::ZERO)))?;
    let (lv, li, lf) = (v.to_bellman(), inv.to_bellman(), flag.to_bellman());
    cs.enforce(|| "v*inv = flag", |lc| lc + &lv, |lc| lc + &li, |lc| lc + &lf);
    Ok(())
}

/// Allocate a boolean witness bit and return it as an `Lc`.
pub fn alloc_bit<CS: ConstraintSystem<Scalar>>(
    cs: &mut CS,
    val: Option<bool>,
) -> Result<Lc, SynthesisError> {
    let bit = AllocatedBit::alloc(cs.namespace(|| "bit"), val)?;
    Ok(Lc::var(bit.get_variable(), val.map(|b| if b { Scalar::ONE } else { Scalar::ZERO })))
}

/// Constrain `v` to lie in `[0, 2^nbits)` by bit decomposition.
pub fn range_check<CS: ConstraintSystem<Scalar>>(
    cs: &mut CS,
    v: &Lc,
    nbits: usize,
) -> Result<(), SynthesisError> {
    let repr = v.val.map(|x| x.to_repr());
    let mut sum = Lc::zero();
    let mut pow = Scalar::ONE;
    for i in 0..nbits {
        let bit_val = repr.map(|r| (r.as_ref()[i / 8] >> (i % 8)) & 1 == 1);
        let bit = alloc_bit(&mut cs.namespace(|| format!("bit {i}")), bit_val)?;
        sum = sum.add(&bit.scale(pow));
        pow = pow.double();
    }
    enforce_equal(cs, v, &sum);
    Ok(())
}
