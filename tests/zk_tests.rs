//! Tests for the zero-knowledge layer in isolation: hash, circuits, proofs.

mod common;
use common::*;

use bellman::gadgets::test::TestConstraintSystem;
use bellman::groth16::{create_random_proof, Proof};
use bellman::{Circuit, ConstraintSystem};
use bls12_381::Scalar;
use ff::Field;
use fleetchain::game::ShotRecord;
use fleetchain::zk::circuits::{
    commit, derive_segments, Segment, SetupCircuit, SetupWitness, ShipPlacement, ShotCircuit,
    ShotWitness,
};
use fleetchain::zk::gadgets::{alloc, alloc_input, enforce_nonzero_flag};
use fleetchain::zk::poseidon;
use fleetchain::zk::{
    scalar_from_hex, scalar_to_hex, FleetSecret, Verifier, ZkSystem, FLEET_CELLS, PROOF_BYTES,
};
use rand::rngs::OsRng;

fn rnd() -> Scalar {
    Scalar::random(&mut OsRng)
}

fn std_placements() -> Vec<ShipPlacement> {
    vec![
        ShipPlacement {
            x: 0,
            y: 0,
            vertical: true,
        }, // carrier  (0,0)-(0,3)
        ShipPlacement {
            x: 2,
            y: 0,
            vertical: true,
        }, // cruiser  (2,0)-(2,2)
        ShipPlacement {
            x: 4,
            y: 0,
            vertical: true,
        }, // sub      (4,0)-(4,1)
        ShipPlacement {
            x: 6,
            y: 0,
            vertical: false,
        }, // destroyer (6,0)
    ]
}

fn shot(index: u64, x: u8, y: u8) -> ShotRecord {
    ShotRecord {
        index,
        shooter: "attacker".into(),
        x,
        y,
        fired_at_block: 0,
    }
}

// ------------------------------------------------------------ poseidon ---

#[test]
fn poseidon_gadget_matches_native_for_all_input_lengths() {
    for len in [0usize, 1, 2, 3, 7, 31] {
        let inputs: Vec<Scalar> = (0..len).map(|_| rnd()).collect();
        let expected = poseidon::hash(&inputs);

        let mut cs = TestConstraintSystem::<Scalar>::new();
        let vars: Vec<_> = inputs
            .iter()
            .enumerate()
            .map(|(i, v)| alloc(&mut cs.namespace(|| format!("in{i}")), Some(*v)).unwrap())
            .collect();
        let out = poseidon::hash_gadget(&mut cs, &vars).unwrap();

        assert!(
            cs.is_satisfied(),
            "len {len}: {:?}",
            cs.which_is_unsatisfied()
        );
        assert_eq!(
            out.value().unwrap(),
            expected,
            "len {len}: gadget != native"
        );
    }
}

#[test]
fn poseidon_is_input_and_length_sensitive() {
    let (a, b) = (Scalar::from(1u64), Scalar::from(2u64));
    assert_ne!(
        poseidon::hash(&[a, b]),
        poseidon::hash(&[b, a]),
        "order matters"
    );
    assert_ne!(
        poseidon::hash(&[a]),
        poseidon::hash(&[a, Scalar::ZERO]),
        "zero padding is not ambiguous"
    );
    assert_ne!(
        poseidon::hash(&[a, b]),
        poseidon::hash(&[a, b, Scalar::ZERO])
    );
    assert_eq!(
        poseidon::hash(&[a, b]),
        poseidon::hash(&[a, b]),
        "deterministic"
    );
}

/// Regression vector: pins the (self-generated) round constants and MDS
/// matrix so any accidental parameter change is caught. Not an external
/// reference vector.
#[test]
fn poseidon_regression_vector() {
    let h = poseidon::hash(&[Scalar::from(1u64), Scalar::from(2u64)]);
    assert_eq!(scalar_to_hex(&h), POSEIDON_1_2);
}
const POSEIDON_1_2: &str = "949de73af35918b18e3cee8437426a531be7eb320d0d8141624a119d2c3aaf71";

// ------------------------------------------------------- setup circuit ---

fn setup_cs(
    placements: Vec<ShipPlacement>,
    board: u8,
    claim_board: u8,
) -> TestConstraintSystem<Scalar> {
    let salt = rnd();
    let c = commit(salt, &derive_segments(&placements));
    let mut cs = TestConstraintSystem::<Scalar>::new();
    SetupCircuit {
        commitment: Some(c),
        board_size: Some(claim_board),
        witness: Some(SetupWitness { salt, placements }),
    }
    .synthesize(&mut cs)
    .unwrap();
    let _ = board;
    cs
}

#[test]
fn setup_circuit_accepts_legal_fleet() {
    let cs = setup_cs(std_placements(), 10, 10);
    assert!(cs.is_satisfied(), "{:?}", cs.which_is_unsatisfied());
    println!("setup circuit: {} constraints", cs.num_constraints());
}

#[test]
fn setup_circuit_accepts_fleet_touching_the_last_row_and_column() {
    // Carrier ends exactly on the board edge in both directions.
    let p = vec![
        ShipPlacement {
            x: 6,
            y: 9,
            vertical: false,
        }, // x 6..=9, y 9
        ShipPlacement {
            x: 9,
            y: 7,
            vertical: true,
        }, // x 9, y 7..=9
        ShipPlacement {
            x: 8,
            y: 9,
            vertical: false,
        }, // x 8..=9
        ShipPlacement {
            x: 9,
            y: 9,
            vertical: false,
        },
    ];
    assert!(setup_cs(p, 10, 10).is_satisfied());
}

#[test]
fn setup_circuit_rejects_ship_hanging_off_the_board() {
    let mut p = std_placements();
    p[0] = ShipPlacement {
        x: 0,
        y: 8,
        vertical: true,
    }; // y 8..=11 on a 10x10 board
    assert!(!setup_cs(p, 10, 10).is_satisfied());
}

#[test]
fn setup_circuit_rejects_horizontal_overhang() {
    let mut p = std_placements();
    p[1] = ShipPlacement {
        x: 8,
        y: 0,
        vertical: false,
    }; // x 8..=10
    assert!(!setup_cs(p, 10, 10).is_satisfied());
}

#[test]
fn setup_circuit_binds_board_size_as_public_input() {
    // Legal on a 20x20 board (a ship at x=15) but illegal on 10x10.
    let mut p = std_placements();
    p[3] = ShipPlacement {
        x: 15,
        y: 0,
        vertical: false,
    };
    assert!(setup_cs(p.clone(), 20, 20).is_satisfied());
    assert!(!setup_cs(p, 20, 10).is_satisfied());
}

#[test]
fn setup_circuit_rejects_commitment_to_a_different_fleet() {
    let salt = rnd();
    let other = derive_segments(&std_placements());
    let c = commit(salt, &other);
    let mut p = std_placements();
    p[3].x = 7; // fleet the prover actually holds differs from what was committed
    let mut cs = TestConstraintSystem::<Scalar>::new();
    SetupCircuit {
        commitment: Some(c),
        board_size: Some(10),
        witness: Some(SetupWitness {
            salt,
            placements: p,
        }),
    }
    .synthesize(&mut cs)
    .unwrap();
    assert!(!cs.is_satisfied());
}

#[test]
fn setup_circuit_allows_overlapping_ships() {
    // Ships may share cells: put the destroyer on the carrier.
    let mut p = std_placements();
    p[3] = ShipPlacement {
        x: 0,
        y: 1,
        vertical: false,
    };
    assert!(setup_cs(p, 10, 10).is_satisfied());
}

// -------------------------------------------------------- shot circuit ---

struct ShotCase {
    segments: Vec<Segment>,
    salt: Scalar,
}

impl ShotCase {
    fn new() -> Self {
        ShotCase {
            segments: derive_segments(&std_placements()),
            salt: rnd(),
        }
    }

    /// Build a shot circuit with arbitrary (possibly dishonest) claims.
    fn circuit(
        &self,
        x: u8,
        y: u8,
        claim_hit: bool,
        claim_alive: bool,
        new_state: &[Segment],
    ) -> ShotCircuit {
        let new_salt = rnd();
        ShotCircuit {
            old_commitment: Some(commit(self.salt, &self.segments)),
            new_commitment: Some(commit(new_salt, new_state)),
            x: Some(x),
            y: Some(y),
            hit: Some(claim_hit),
            alive: Some(claim_alive),
            witness: Some(ShotWitness {
                old_salt: self.salt,
                new_salt,
                segments: self.segments.clone(),
            }),
        }
    }

    fn honest_next(&self, x: u8, y: u8) -> (Vec<Segment>, bool, bool) {
        let mut next = self.segments.clone();
        let mut hit = false;
        for s in next.iter_mut() {
            if s.alive && (s.x, s.y) == (x, y) {
                s.alive = false;
                hit = true;
            }
        }
        let alive = next.iter().any(|s| s.alive);
        (next, hit, alive)
    }
}

fn satisfied(c: ShotCircuit) -> bool {
    let mut cs = TestConstraintSystem::<Scalar>::new();
    c.synthesize(&mut cs).unwrap();
    cs.is_satisfied()
}

#[test]
fn shot_circuit_accepts_honest_hit_and_honest_miss() {
    let case = ShotCase::new();
    for (x, y, expect_hit) in [(0u8, 2u8, true), (6, 0, true), (9, 9, false), (1, 0, false)] {
        let (next, hit, alive) = case.honest_next(x, y);
        assert_eq!(hit, expect_hit);
        assert!(
            satisfied(case.circuit(x, y, hit, alive, &next)),
            "({x},{y})"
        );
    }
    let mut cs = TestConstraintSystem::<Scalar>::new();
    let (next, hit, alive) = case.honest_next(0, 0);
    case.circuit(0, 0, hit, alive, &next)
        .synthesize(&mut cs)
        .unwrap();
    println!("shot circuit: {} constraints", cs.num_constraints());
}

#[test]
fn shot_circuit_rejects_claiming_miss_on_a_real_hit() {
    let case = ShotCase::new();
    let (next, _hit, alive) = case.honest_next(0, 1);
    assert!(!satisfied(case.circuit(0, 1, false, alive, &next)));
    // ... even if the liar keeps the state unchanged so the commitments line up
    assert!(!satisfied(case.circuit(
        0,
        1,
        false,
        true,
        &case.segments.clone()
    )));
}

#[test]
fn shot_circuit_rejects_claiming_hit_on_a_miss() {
    let case = ShotCase::new();
    let (next, _hit, alive) = case.honest_next(9, 9);
    assert!(!satisfied(case.circuit(9, 9, true, alive, &next)));
}

#[test]
fn shot_circuit_rejects_a_hit_that_does_not_kill_the_segment() {
    // Honest hit/alive bits but the new commitment keeps the segment alive,
    // which would let the same cell be hit again and again.
    let case = ShotCase::new();
    let (_next, hit, alive) = case.honest_next(6, 0);
    assert!(hit);
    assert!(!satisfied(case.circuit(
        6,
        0,
        hit,
        alive,
        &case.segments.clone()
    )));
}

#[test]
fn shot_circuit_rejects_moving_a_ship_in_the_new_state() {
    let case = ShotCase::new();
    let (mut next, hit, alive) = case.honest_next(9, 9);
    next[0].x = 5; // teleport a segment while "missing"
    assert!(!satisfied(case.circuit(9, 9, hit, alive, &next)));
}

#[test]
fn shot_circuit_rejects_resurrecting_a_dead_segment() {
    let mut case = ShotCase::new();
    case.segments[3].alive = false; // carrier tail (0,3) already sunk
    let mut resurrected = case.segments.clone();
    resurrected[3].alive = true;
    assert!(!satisfied(case.circuit(9, 9, false, true, &resurrected)));
}

#[test]
fn shot_circuit_rejects_lying_about_being_alive() {
    // Sink everything but one cell, then hit it: alive must be false.
    let mut case = ShotCase::new();
    for s in case.segments.iter_mut() {
        s.alive = false;
    }
    case.segments[9] = Segment {
        x: 6,
        y: 0,
        alive: true,
    };
    let (next, hit, alive) = case.honest_next(6, 0);
    assert!(hit && !alive);
    assert!(satisfied(case.circuit(6, 0, hit, false, &next)));
    assert!(
        !satisfied(case.circuit(6, 0, hit, true, &next)),
        "claiming alive after the last hit"
    );
}

#[test]
fn shot_circuit_dead_segments_do_not_report_hits() {
    // Re-shooting a sunk cell is a miss (and cannot double count damage).
    let mut case = ShotCase::new();
    case.segments[9].alive = false; // destroyer at (6,0) already sunk
    let (next, hit, alive) = case.honest_next(6, 0);
    assert!(!hit);
    assert!(satisfied(case.circuit(6, 0, false, alive, &next)));
    assert!(!satisfied(case.circuit(6, 0, true, alive, &next)));
}

#[test]
fn shot_circuit_kills_every_ship_stacked_on_a_cell() {
    // Destroyer placed on top of the carrier's (0,1): one shot sinks both cells.
    let mut p = std_placements();
    p[3] = ShipPlacement {
        x: 0,
        y: 1,
        vertical: false,
    };
    let case = ShotCase {
        segments: derive_segments(&p),
        salt: rnd(),
    };
    let (next, hit, alive) = case.honest_next(0, 1);
    assert!(hit && alive);
    assert_eq!(next.iter().filter(|s| s.alive).count(), FLEET_CELLS - 2);
    assert!(satisfied(case.circuit(0, 1, hit, alive, &next)));
}

#[test]
fn shot_circuit_rejects_wrong_old_salt() {
    let case = ShotCase::new();
    let (next, hit, alive) = case.honest_next(0, 0);
    let mut c = case.circuit(0, 0, hit, alive, &next);
    c.witness.as_mut().unwrap().old_salt = rnd();
    assert!(!satisfied(c));
}

#[test]
fn flag_gadget_rejects_non_boolean_public_flag() {
    // hit = 2 with v != 0 would satisfy v * inv = flag if flag were not
    // constrained to {0,1}; make sure it is.
    let mut cs = TestConstraintSystem::<Scalar>::new();
    let v = alloc(&mut cs.namespace(|| "v"), Some(Scalar::from(5u64))).unwrap();
    let flag = alloc_input(&mut cs.namespace(|| "flag"), Some(Scalar::from(2u64))).unwrap();
    enforce_nonzero_flag(&mut cs.namespace(|| "f"), &v, &flag).unwrap();
    assert!(!cs.is_satisfied());

    let mut ok = TestConstraintSystem::<Scalar>::new();
    let v = alloc(&mut ok.namespace(|| "v"), Some(Scalar::from(5u64))).unwrap();
    let flag = alloc_input(&mut ok.namespace(|| "flag"), Some(Scalar::ONE)).unwrap();
    enforce_nonzero_flag(&mut ok.namespace(|| "f"), &v, &flag).unwrap();
    assert!(ok.is_satisfied());
}

// ---------------------------------------------------- real Groth16 proofs ---

fn setup_player(board: u8) -> FleetSecret {
    FleetSecret::from_ships(&valid_fleet(), board).unwrap()
}

#[test]
fn groth16_setup_proof_verifies() {
    let s = setup_player(10);
    let proof = s.prove_setup(sys()).unwrap();
    assert_eq!(proof.len(), PROOF_BYTES);
    sys()
        .verifier
        .verify_setup(&s.commitment(), 10, &proof)
        .unwrap();
}

#[test]
fn groth16_setup_proof_is_bound_to_commitment_and_board_size() {
    let s = setup_player(10);
    let other = setup_player(10);
    let proof = s.prove_setup(sys()).unwrap();
    let v = &sys().verifier;
    assert!(
        v.verify_setup(&other.commitment(), 10, &proof).is_err(),
        "someone else's commitment"
    );
    assert!(
        v.verify_setup(&s.commitment(), 11, &proof).is_err(),
        "different board size"
    );
    assert!(
        v.verify_setup(&s.commitment(), 9, &proof).is_err(),
        "different board size"
    );
}

#[test]
fn groth16_malicious_prover_cannot_prove_an_off_board_fleet() {
    // Bypass the client-side checks and run the real prover on an illegal witness.
    let mut p = std_placements();
    p[0] = ShipPlacement {
        x: 0,
        y: 8,
        vertical: true,
    }; // sticks out of a 10x10 board
    let salt = rnd();
    let c = commit(salt, &derive_segments(&p));
    let proof = create_random_proof(
        SetupCircuit {
            commitment: Some(c),
            board_size: Some(10),
            witness: Some(SetupWitness {
                salt,
                placements: p,
            }),
        },
        &sys().setup_params,
        &mut OsRng,
    )
    .unwrap();
    let mut bytes = Vec::new();
    proof.write(&mut bytes).unwrap();
    assert!(sys().verifier.verify_setup(&c, 10, &bytes).is_err());
}

#[test]
fn groth16_full_game_against_one_defender() {
    let mut s = setup_player(10);
    let v = &sys().verifier;
    let mut remaining = 10;
    let mut idx = 0;

    // Miss first.
    let r = s.respond(sys(), "d", &shot(idx, 9, 9)).unwrap();
    let old = s.commitment();
    let new = scalar_from_hex(&r.report.new_commitment).unwrap();
    assert!(!r.report.is_hit && r.report.is_alive);
    v.verify_shot(&old, &new, 9, 9, false, true, &r.report.proof)
        .unwrap();
    s.apply(r);
    assert_eq!(s.remaining_cells(), remaining);
    idx += 1;

    // Then sink every cell of the fleet.
    for (x, y) in cells(&valid_fleet()) {
        let old = s.commitment();
        let r = s.respond(sys(), "d", &shot(idx, x, y)).unwrap();
        let new = scalar_from_hex(&r.report.new_commitment).unwrap();
        assert!(r.report.is_hit, "({x},{y}) should hit");
        v.verify_shot(&old, &new, x, y, true, r.report.is_alive, &r.report.proof)
            .unwrap();
        s.apply(r);
        remaining -= 1;
        assert_eq!(s.remaining_cells(), remaining);
        idx += 1;
    }
    assert!(s.is_defeated());
}

#[test]
fn groth16_shot_proof_cannot_be_reused_or_altered() {
    let mut s = setup_player(10);
    let v = &sys().verifier;
    let old = s.commitment();
    let r = s.respond(sys(), "d", &shot(0, 0, 1)).unwrap(); // a hit
    let new = scalar_from_hex(&r.report.new_commitment).unwrap();
    let proof = r.report.proof.clone();
    v.verify_shot(&old, &new, 0, 1, true, true, &proof).unwrap();

    // Flip the claimed result, the coordinates, either commitment.
    assert!(
        v.verify_shot(&old, &new, 0, 1, false, true, &proof)
            .is_err(),
        "flipped hit"
    );
    assert!(
        v.verify_shot(&old, &new, 0, 1, true, false, &proof)
            .is_err(),
        "flipped alive"
    );
    assert!(
        v.verify_shot(&old, &new, 0, 2, true, true, &proof).is_err(),
        "different cell"
    );
    assert!(
        v.verify_shot(&old, &new, 1, 1, true, true, &proof).is_err(),
        "different cell"
    );
    assert!(
        v.verify_shot(&new, &old, 0, 1, true, true, &proof).is_err(),
        "swapped commitments"
    );
    assert!(
        v.verify_shot(&old, &rnd(), 0, 1, true, true, &proof)
            .is_err(),
        "other new commitment"
    );

    // Tamper with the proof bytes.
    for pos in [0usize, 47, 48, 100, 143, 144, 191] {
        let mut bad = proof.clone();
        bad[pos] ^= 0x01;
        assert!(
            v.verify_shot(&old, &new, 0, 1, true, true, &bad).is_err(),
            "bit flip at {pos}"
        );
    }
    // Wrong lengths.
    assert!(v
        .verify_shot(&old, &new, 0, 1, true, true, &proof[..191])
        .is_err());
    let mut longer = proof.clone();
    longer.push(0);
    assert!(v
        .verify_shot(&old, &new, 0, 1, true, true, &longer)
        .is_err());
    assert!(v.verify_shot(&old, &new, 0, 1, true, true, &[]).is_err());

    // After the state advanced the proof is stale: it opens the *old* commitment.
    s.apply(r);
    let next = s.respond(sys(), "d", &shot(1, 0, 1)).unwrap();
    let next_new = scalar_from_hex(&next.report.new_commitment).unwrap();
    assert!(
        v.verify_shot(&s.commitment(), &next_new, 0, 1, true, true, &proof)
            .is_err(),
        "replay"
    );
    // Re-shooting the same cell is now a miss.
    assert!(!next.report.is_hit);
}

#[test]
fn groth16_malicious_prover_cannot_report_a_miss_on_a_hit() {
    // Use the real proving key with a lying claim. The prover runs, but the
    // resulting proof must not verify.
    let case = ShotCase::new();
    let (next, _, alive) = case.honest_next(0, 0);
    let c = case.circuit(0, 0, false, alive, &next); // claims MISS on a real hit
    let (old, new) = (c.old_commitment.unwrap(), c.new_commitment.unwrap());
    let proof = create_random_proof(c, &sys().shot_params, &mut OsRng).unwrap();
    let mut bytes = Vec::new();
    proof.write(&mut bytes).unwrap();
    assert!(sys()
        .verifier
        .verify_shot(&old, &new, 0, 0, false, alive, &bytes)
        .is_err());
    // and it does not verify as a "hit" either
    assert!(sys()
        .verifier
        .verify_shot(&old, &new, 0, 0, true, alive, &bytes)
        .is_err());
    let _ = Proof::<bls12_381::Bls12>::read(&bytes[..]).unwrap();
}

#[test]
fn respond_does_not_advance_state_until_applied() {
    let mut s = setup_player(10);
    let before = s.commitment();
    let first = s.respond(sys(), "d", &shot(0, 0, 0)).unwrap();
    assert_eq!(s.commitment(), before);
    assert_eq!(s.remaining_cells(), 10);
    // A retry from the same state yields a fresh, independent response.
    let retry = s.respond(sys(), "d", &shot(0, 0, 0)).unwrap();
    assert_ne!(
        first.report.new_commitment, retry.report.new_commitment,
        "fresh salt per attempt"
    );
    s.apply(first);
    assert_ne!(s.commitment(), before);
    assert_eq!(s.remaining_cells(), 9);
}

#[test]
fn client_rejects_fleets_that_do_not_fit_the_board() {
    let ships = valid_fleet(); // uses x up to 6, y up to 3
    assert!(FleetSecret::from_ships(&ships, 7).is_ok());
    assert!(FleetSecret::from_ships(&ships, 6).is_err());
    let err = FleetSecret::from_ships(&ships, 3).unwrap_err();
    assert!(err.contains("outside"), "{err}");
}

// --------------------------------------------------------- key handling ---

#[test]
fn verifier_keys_roundtrip_and_fingerprint_is_stable() {
    let mut buf = Vec::new();
    sys().verifier.write(&mut buf).unwrap();
    let loaded = Verifier::read(&buf[..]).unwrap();
    assert_eq!(loaded.fingerprint(), sys().verifier.fingerprint());

    let s = setup_player(10);
    let proof = s.prove_setup(sys()).unwrap();
    loaded.verify_setup(&s.commitment(), 10, &proof).unwrap();

    assert!(Verifier::read(&b"not a key file"[..]).is_err());
    buf[0] ^= 0xff;
    assert!(Verifier::read(&buf[..]).is_err());
}

#[test]
fn independent_setups_do_not_accept_each_others_proofs() {
    let other = ZkSystem::generate(&mut OsRng).unwrap();
    assert_ne!(other.verifier.fingerprint(), sys().verifier.fingerprint());
    let s = setup_player(10);
    let proof = s.prove_setup(sys()).unwrap();
    assert!(other
        .verifier
        .verify_setup(&s.commitment(), 10, &proof)
        .is_err());
}

#[test]
fn scalar_hex_roundtrip_and_validation() {
    let x = rnd();
    assert_eq!(scalar_from_hex(&scalar_to_hex(&x)).unwrap(), x);
    assert!(scalar_from_hex("zz").is_err());
    assert!(scalar_from_hex("00").is_err(), "wrong length");
    assert!(
        scalar_from_hex(&"ff".repeat(32)).is_err(),
        "not reduced mod r"
    );
}

// ------------------------------------------- secret files & random fleets ---

#[test]
fn random_fleets_are_always_legal_even_on_the_smallest_board() {
    for board in [4u8, 5, 10, 50, 255] {
        for _ in 0..40 {
            let f = FleetSecret::random(board).unwrap_or_else(|e| panic!("board {board}: {e}"));
            assert_eq!(f.board_size(), board);
            assert_eq!(f.remaining_cells(), FLEET_CELLS);
        }
    }
    assert!(FleetSecret::random(3).is_err());
}

#[test]
fn random_fleet_registers_with_a_real_proof() {
    let f = FleetSecret::random(10).unwrap();
    let proof = f.prove_setup(sys()).unwrap();
    sys()
        .verifier
        .verify_setup(&f.commitment(), 10, &proof)
        .unwrap();
}

#[test]
fn secret_file_roundtrip_preserves_state_and_keeps_working() {
    let mut s = setup_player(10);
    let r = s.respond(sys(), "d", &shot(0, 0, 1)).unwrap(); // a hit
    s.apply(r);

    let json = s.to_json();
    let mut loaded = FleetSecret::from_json(&json).unwrap();
    assert_eq!(loaded.commitment(), s.commitment());
    assert_eq!(loaded.remaining_cells(), 9);

    // The restored secret can keep playing and its proofs verify.
    let old = loaded.commitment();
    let r = loaded.respond(sys(), "d", &shot(1, 0, 2)).unwrap();
    let new = scalar_from_hex(&r.report.new_commitment).unwrap();
    assert!(r.report.is_hit);
    sys()
        .verifier
        .verify_shot(&old, &new, 0, 2, true, true, &r.report.proof)
        .unwrap();
    loaded.apply(r);
    assert_eq!(loaded.remaining_cells(), 8);

    // Debug output never contains the fleet.
    let dbg = format!("{loaded:?}");
    assert!(dbg.contains("redacted") && !dbg.contains("placements") && !dbg.contains("salt"));
}

#[test]
fn corrupted_or_edited_secret_files_are_rejected() {
    let json = setup_player(10).to_json();
    let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();

    let mut bad = v.clone();
    bad["salt"] = serde_json::Value::String(scalar_to_hex(&rnd()));
    assert!(FleetSecret::from_json(&bad.to_string())
        .unwrap_err()
        .contains("corrupted"));

    let mut bad = v.clone();
    bad["segments"][0]["alive"] = serde_json::Value::Bool(false); // resurrect/kill by hand
    assert!(FleetSecret::from_json(&bad.to_string()).is_err());

    let mut bad = v.clone();
    bad["segments"][0]["x"] = serde_json::json!(9); // positions no longer match placements
    assert!(FleetSecret::from_json(&bad.to_string())
        .unwrap_err()
        .contains("inconsistent"));

    let mut bad = v.clone();
    bad["board_size"] = serde_json::json!(3); // fleet no longer fits
    assert!(FleetSecret::from_json(&bad.to_string()).is_err());

    v["segments"].as_array_mut().unwrap().pop();
    assert!(FleetSecret::from_json(&v.to_string()).is_err());
    assert!(FleetSecret::from_json("{").is_err());
}
