//! Walkthrough of the zero-knowledge hit/miss protocol.
//!
//!     cargo run --example zk_proof_demo
//!
//! One defender with a secret fleet, one verifier that only ever sees
//! commitments, result bits and proofs.

use fleetchain::game::{Ship, ShotRecord};
use fleetchain::zk::{scalar_from_hex, FleetSecret, ZkSystem};
use std::time::Instant;

fn shot(index: u64, x: u8, y: u8) -> ShotRecord {
    ShotRecord {
        index,
        shooter: "attacker".into(),
        x,
        y,
        fired_at_block: 0,
    }
}

fn main() {
    println!("=== FleetChain zero-knowledge battleship ===\n");

    let t = Instant::now();
    let sys = ZkSystem::generate(&mut rand::rngs::OsRng).expect("setup");
    println!(
        "Groth16 setup (INSECURE single-party, demo only): {:.1?}",
        t.elapsed()
    );
    println!(
        "Verifying-key fingerprint: {}...\n",
        &sys.verifier.fingerprint()[..16]
    );
    let verifier = &sys.verifier;

    // ---- Defender's secret fleet (never leaves this scope) ----
    let ships = vec![
        Ship::new("Carrier".into(), vec![(0, 0), (0, 1), (0, 2), (0, 3)]),
        Ship::new("Cruiser".into(), vec![(2, 0), (2, 1), (2, 2)]),
        Ship::new("Submarine".into(), vec![(4, 0), (4, 1)]),
        Ship::new("Destroyer".into(), vec![(6, 0)]),
    ];
    let mut fleet = FleetSecret::from_ships(&ships, 10).expect("legal fleet");

    // ---- 1. Registration: commitment + proof that it hides a legal fleet ----
    let t = Instant::now();
    let setup_proof = fleet.prove_setup(&sys).expect("prove setup");
    println!("1. Registration");
    println!("   commitment : {}", fleet.commitment_hex());
    println!(
        "   proof      : {} bytes (proved in {:.1?})",
        setup_proof.len(),
        t.elapsed()
    );
    verifier
        .verify_setup(&fleet.commitment(), 10, &setup_proof)
        .expect("setup verifies");
    println!("   verifier   : legal fleet on a 10x10 board ✓ (positions unknown to verifier)\n");

    // ---- 2. A miss ----
    println!("2. Attacker fires at (9, 9)");
    let old = fleet.commitment();
    let t = Instant::now();
    let resp = fleet
        .respond(&sys, "defender", &shot(0, 9, 9))
        .expect("respond");
    let new = scalar_from_hex(&resp.report.new_commitment).unwrap();
    println!(
        "   defender says: {} (proved in {:.1?})",
        if resp.report.is_hit { "HIT" } else { "miss" },
        t.elapsed()
    );
    verifier
        .verify_shot(
            &old,
            &new,
            9,
            9,
            resp.report.is_hit,
            resp.report.is_alive,
            &resp.report.proof,
        )
        .expect("verifies");
    println!("   verifier: proof checks out ✓\n");
    fleet.apply(resp);

    // ---- 3. A hit, and a lying defender ----
    println!("3. Attacker fires at (0, 1) -- a real ship cell");
    let old = fleet.commitment();
    let resp = fleet
        .respond(&sys, "defender", &shot(1, 0, 1))
        .expect("respond");
    let new = scalar_from_hex(&resp.report.new_commitment).unwrap();
    println!(
        "   defender says: {}",
        if resp.report.is_hit { "HIT" } else { "miss" }
    );
    verifier
        .verify_shot(&old, &new, 0, 1, true, true, &resp.report.proof)
        .expect("verifies");
    println!("   verifier: honest HIT proof checks out ✓");
    let lie = verifier.verify_shot(&old, &new, 0, 1, false, true, &resp.report.proof);
    println!(
        "   verifier: same proof presented as a 'miss' -> {}",
        if lie.is_err() {
            "REJECTED ✓"
        } else {
            "accepted ✗✗"
        }
    );
    let other_cell = verifier.verify_shot(&old, &new, 0, 2, true, true, &resp.report.proof);
    println!(
        "   verifier: same proof presented for cell (0,2) -> {}",
        if other_cell.is_err() {
            "REJECTED ✓"
        } else {
            "accepted ✗✗"
        }
    );
    fleet.apply(resp);

    // ---- 4. Damage persists ----
    println!("\n4. Attacker fires at (0, 1) again");
    let resp = fleet
        .respond(&sys, "defender", &shot(2, 0, 1))
        .expect("respond");
    println!(
        "   defender says: {} (that segment is already sunk)",
        if resp.report.is_hit { "HIT" } else { "miss" }
    );
    fleet.apply(resp);

    println!(
        "\nSecretly the defender still has {} of 10 cells afloat.",
        fleet.remaining_cells()
    );
    println!("The verifier learned one bit per shot and nothing else.");
}
