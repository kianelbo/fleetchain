//! End-to-end protocol tests: coordinator + players + real Groth16 proofs.

mod common;
use common::*;

use bellman::groth16::create_random_proof;
use bls12_381::Scalar;
use ff::Field;
use fleetchain::coordinator::GameCoordinator;
use fleetchain::game::Ship;
use fleetchain::zk::circuits::{
    commit, derive_segments, SetupCircuit, SetupWitness, ShipPlacement, ShotCircuit, ShotWitness,
};
use fleetchain::zk::{scalar_to_hex, FleetSecret};
use rand::rngs::OsRng;

/// A fleet in the lower-right corner, disjoint from `valid_fleet()`.
fn far_fleet() -> Vec<Ship> {
    vec![
        Ship::new("Carrier".into(), vec![(9, 5), (9, 6), (9, 7), (9, 8)]),
        Ship::new("Cruiser".into(), vec![(7, 5), (7, 6), (7, 7)]),
        Ship::new("Submarine".into(), vec![(5, 5), (5, 6)]),
        Ship::new("Destroyer".into(), vec![(3, 9)]),
    ]
}

fn game() -> GameCoordinator {
    GameCoordinator::new(10, 1)
}

// ------------------------------------------------------- registration ---

#[test]
fn registration_stores_only_public_data() {
    let mut c = game();
    let s = register_default(&mut c, "alice");
    let p = &c.players["alice"];
    assert_eq!(p.commitment, s.commitment());
    assert_eq!(p.initial_commitment, s.commitment());
    assert!(p.alive && !p.forfeited);
    assert!(c.grid.cells.is_empty(), "the shared grid must not record who is where");
    assert_eq!(c.blockchain.get_unspent_shots("alice"), 1);
}

#[test]
fn duplicate_player_ids_are_rejected() {
    let mut c = game();
    register_default(&mut c, "alice");
    let err = register(&mut c, "alice", &far_fleet()).unwrap_err();
    assert!(err.contains("already registered"), "{err}");
    assert_eq!(c.players.len(), 1);
}

#[test]
fn someone_elses_registration_cannot_be_copied() {
    // Copying a victim's commitment + proof under a new name would let the
    // copycat mirror the victim's (public) responses.
    let mut c = game();
    let victim = FleetSecret::from_ships(&valid_fleet(), 10).unwrap();
    let proof = victim.prove_setup(sys()).unwrap();
    c.register_player("victim".into(), victim.commitment_hex(), &proof).unwrap();
    let err = c.register_player("copycat".into(), victim.commitment_hex(), &proof).unwrap_err();
    assert!(err.contains("already registered") || err.contains("Commitment"), "{err}");
    assert_eq!(c.players.len(), 1);
}

#[test]
fn registration_proof_is_tied_to_the_board_size() {
    // Proven for a 10x10 board, submitted to a 20x20 game (and vice versa).
    let secret = FleetSecret::from_ships(&valid_fleet(), 10).unwrap();
    let proof = secret.prove_setup(sys()).unwrap();
    let mut big = GameCoordinator::new(20, 1);
    assert!(big.register_player("p".into(), secret.commitment_hex(), &proof).is_err());
    assert_eq!(big.players.len(), 0);
}

#[test]
fn malicious_player_cannot_register_a_fleet_that_hangs_off_the_board() {
    // Skip the client-side checks and run the real prover on an illegal fleet.
    let placements = vec![
        ShipPlacement { x: 0, y: 8, vertical: true }, // y = 8..=11
        ShipPlacement { x: 2, y: 0, vertical: true },
        ShipPlacement { x: 4, y: 0, vertical: true },
        ShipPlacement { x: 6, y: 0, vertical: false },
    ];
    let salt = Scalar::random(&mut OsRng);
    let c0 = commit(salt, &derive_segments(&placements));
    let proof = create_random_proof(
        SetupCircuit { commitment: Some(c0), board_size: Some(10), witness: Some(SetupWitness { salt, placements }) },
        &sys().setup_params,
        &mut OsRng,
    )
    .unwrap();
    let mut bytes = Vec::new();
    proof.write(&mut bytes).unwrap();

    let mut c = game();
    assert!(c.register_player("cheat".into(), scalar_to_hex(&c0), &bytes).is_err());
    assert_eq!(c.players.len(), 0);
}

// ------------------------------------------------------- shot/response ---

#[test]
fn every_other_player_owes_a_response_but_the_shooter_does_not() {
    let mut c = game();
    register_default(&mut c, "a");
    register(&mut c, "b", &far_fleet()).unwrap();
    register(&mut c, "c", &horizontal_fleet()).unwrap();

    c.fire_shot("a".into(), 4, 4).unwrap();
    assert_eq!(c.pending_shots("a").len(), 0);
    assert_eq!(c.pending_shots("b").len(), 1);
    assert_eq!(c.pending_shots("c").len(), 1);
    assert_eq!(c.shot_log.len(), 1);
    assert_eq!((c.shot_log[0].x, c.shot_log[0].y, c.shot_log[0].shooter.as_str()), (4, 4, "a"));
}

#[test]
fn hit_and_miss_are_reported_per_defender_and_ledger_has_only_bits() {
    let mut c = game();
    register(&mut c, "a", &far_fleet()).unwrap();
    let mut b = register(&mut c, "b", &valid_fleet()).unwrap(); // has (0,1)
    let mut d = register(&mut c, "d", &horizontal_fleet()).unwrap(); // does not

    c.fire_shot("a".into(), 0, 1).unwrap();
    assert_eq!(answer_next(&mut c, &mut b, "b"), (true, true));
    assert_eq!(answer_next(&mut c, &mut d, "d"), (false, true));

    let results = c.shot_results(0);
    assert_eq!(results.len(), 2);
    assert!(results.iter().any(|r| r.player_id == "b" && r.is_hit));
    assert!(results.iter().any(|r| r.player_id == "d" && !r.is_hit));
    assert_eq!(c.players["b"].hits_taken, 1);
    assert_eq!(c.players["d"].hits_taken, 0);
    assert_eq!(b.remaining_cells(), 9);

    // The public ledger carries nothing but result bits and commitments.
    let json = serde_json::to_value(&c.responses[0]).unwrap();
    let mut keys: Vec<_> = json.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    assert_eq!(keys, ["is_alive", "is_hit", "new_commitment", "player_id", "shot_index"]);
    // Coordinator state tracks the defender's new commitment.
    assert_eq!(c.players["b"].commitment, b.commitment());
}

#[test]
fn several_players_with_ships_on_one_cell_all_report_a_hit() {
    // Ships of different players share cells; one shot hits all of them.
    let mut c = game();
    register(&mut c, "a", &far_fleet()).unwrap();
    let mut b = register(&mut c, "b", &valid_fleet()).unwrap();
    let mut d = register(&mut c, "d", &valid_fleet()).unwrap(); // identical layout
    c.fire_shot("a".into(), 2, 1).unwrap();
    assert_eq!(answer_next(&mut c, &mut b, "b"), (true, true));
    assert_eq!(answer_next(&mut c, &mut d, "d"), (true, true));
}

#[test]
fn ships_of_one_player_may_overlap_and_one_shot_hits_both() {
    // Destroyer stacked on the carrier's (0,1).
    let stacked = vec![
        Ship::new("Carrier".into(), vec![(0, 0), (0, 1), (0, 2), (0, 3)]),
        Ship::new("Cruiser".into(), vec![(2, 0), (2, 1), (2, 2)]),
        Ship::new("Submarine".into(), vec![(4, 0), (4, 1)]),
        Ship::new("Destroyer".into(), vec![(0, 1)]),
    ];
    let mut c = game();
    register(&mut c, "a", &far_fleet()).unwrap();
    let mut b = register(&mut c, "b", &stacked).unwrap();
    assert_eq!(b.remaining_cells(), 10);

    c.fire_shot("a".into(), 0, 1).unwrap();
    assert_eq!(answer_next(&mut c, &mut b, "b"), (true, true));
    assert_eq!(b.remaining_cells(), 8, "both stacked cells die at once");

    // Shooting the now-empty cell again is a plain miss.
    ensure_shots(&mut c, "a", 1);
    c.fire_shot("a".into(), 0, 1).unwrap();
    assert_eq!(answer_next(&mut c, &mut b, "b"), (false, true));
    assert_eq!(c.players["b"].hits_taken, 1);
}

#[test]
fn responses_must_be_given_in_shot_order() {
    let mut c = game();
    register(&mut c, "a", &far_fleet()).unwrap();
    let mut b = register_default(&mut c, "b");
    ensure_shots(&mut c, "a", 2);
    c.fire_shot("a".into(), 0, 0).unwrap();
    c.fire_shot("a".into(), 9, 9).unwrap();

    // Prove the second shot against the current state and try to submit it first.
    let pending = c.pending_shots("b");
    assert_eq!(pending.len(), 2);
    let early = b.respond(sys(), "b", &pending[1]).unwrap();
    let err = c.report_hit(early.report).unwrap_err();
    assert!(err.contains("Out-of-order"), "{err}");

    answer_next(&mut c, &mut b, "b");
    answer_next(&mut c, &mut b, "b");
    assert!(c.pending_shots("b").is_empty());
}

#[test]
fn a_response_cannot_be_replayed() {
    let mut c = game();
    register(&mut c, "a", &far_fleet()).unwrap();
    let mut b = register_default(&mut c, "b");
    ensure_shots(&mut c, "a", 3);

    // Shot 0: b answers; keep a copy of the accepted report.
    c.fire_shot("a".into(), 0, 0).unwrap();
    let shot0 = c.pending_shots("b")[0].clone();
    let first = b.respond(sys(), "b", &shot0).unwrap();
    let accepted = first.report.clone();
    c.report_hit(accepted.clone()).unwrap();
    b.apply(first);

    // (1) Straight away: nothing is owed any more.
    assert!(c.report_hit(accepted.clone()).is_err());

    // (2) With a new shot pending, relabelling the report is caught by the cheap
    //     freshness check: its "new" commitment is already the current one.
    c.fire_shot("a".into(), 0, 0).unwrap();
    let mut replay = accepted.clone();
    replay.shot_index = 1;
    assert!(c.report_hit(replay).unwrap_err().contains("must differ"));

    // (3) After b has legitimately moved on again, the old proof's commitments
    //     look plausible (new != current) so only the proof itself can reject
    //     it: it opens a commitment that is no longer the current state.
    answer_next(&mut c, &mut b, "b");
    c.fire_shot("a".into(), 0, 0).unwrap();
    let mut replay = accepted.clone();
    replay.shot_index = 2;
    assert!(c.report_hit(replay).unwrap_err().contains("Invalid shot proof"));
}

#[test]
fn defender_cannot_lie_about_hit_or_alive_and_state_is_untouched_by_rejections() {
    let mut c = game();
    register(&mut c, "a", &far_fleet()).unwrap();
    let mut b = register_default(&mut c, "b");
    c.fire_shot("a".into(), 0, 1).unwrap(); // a genuine hit on b
    let shot = c.pending_shots("b")[0].clone();
    let honest = b.respond(sys(), "b", &shot).unwrap();
    assert!(honest.report.is_hit);
    let before = c.players["b"].commitment;

    let mut lie = honest.report.clone();
    lie.is_hit = false;
    assert!(c.report_hit(lie).is_err(), "claiming a miss on a hit");

    let mut lie = honest.report.clone();
    lie.is_alive = false;
    assert!(c.report_hit(lie).is_err(), "claiming to be sunk");

    let mut lie = honest.report.clone();
    lie.shot_x = 5;
    assert!(c.report_hit(lie).is_err(), "answering a different cell");

    let mut lie = honest.report.clone();
    lie.proof[10] ^= 1;
    assert!(c.report_hit(lie).is_err(), "corrupted proof");

    let mut lie = honest.report.clone();
    lie.proof.clear();
    assert!(c.report_hit(lie).is_err(), "empty proof");

    let mut lie = honest.report.clone();
    lie.new_commitment = scalar_to_hex(&before);
    assert!(c.report_hit(lie).unwrap_err().contains("must differ"), "reusing the old commitment");

    let mut lie = honest.report.clone();
    lie.new_commitment = "not hex".into();
    assert!(c.report_hit(lie).is_err());

    // Nothing above changed anything, and the honest answer still goes through.
    assert_eq!(c.players["b"].commitment, before);
    assert!(c.responses.is_empty());
    c.report_hit(honest.report.clone()).unwrap();
    b.apply(honest);
    assert_eq!(c.players["b"].commitment, b.commitment());
}

#[test]
fn proof_of_one_defender_cannot_be_used_by_another() {
    let mut c = game();
    register(&mut c, "a", &far_fleet()).unwrap();
    let b = register(&mut c, "b", &horizontal_fleet()).unwrap();
    let d = register(&mut c, "d", &horizontal_fleet()).unwrap(); // same cells, different salt
    c.fire_shot("a".into(), 9, 9).unwrap();
    let shot = c.pending_shots("b")[0].clone();
    let from_b = b.respond(sys(), "b", &shot).unwrap();
    let mut stolen = from_b.report.clone();
    stolen.player_id = "d".into();
    assert!(c.report_hit(stolen).is_err());
    let _ = d;
}

#[test]
fn malicious_defender_with_real_prover_cannot_report_a_miss_on_a_hit() {
    // The cheater registers legitimately (own salt, known fleet) and then
    // runs the REAL proving key with a lying claim. The proof must not verify.
    let placements = vec![
        ShipPlacement { x: 0, y: 0, vertical: true },
        ShipPlacement { x: 2, y: 0, vertical: true },
        ShipPlacement { x: 4, y: 0, vertical: true },
        ShipPlacement { x: 6, y: 0, vertical: false },
    ];
    let salt = Scalar::random(&mut OsRng);
    let segments = derive_segments(&placements);
    let c0 = commit(salt, &segments);
    let setup = create_random_proof(
        SetupCircuit { commitment: Some(c0), board_size: Some(10), witness: Some(SetupWitness { salt, placements }) },
        &sys().setup_params,
        &mut OsRng,
    )
    .unwrap();
    let mut setup_bytes = Vec::new();
    setup.write(&mut setup_bytes).unwrap();

    let mut c = game();
    register(&mut c, "a", &far_fleet()).unwrap();
    c.register_player("cheat".into(), scalar_to_hex(&c0), &setup_bytes).unwrap();

    c.fire_shot("a".into(), 0, 1).unwrap(); // (0,1) really is a ship cell
    let shot = c.pending_shots("cheat")[0].clone();

    // Lie: "miss", keeping the state unchanged so the commitments look plausible.
    let new_salt = Scalar::random(&mut OsRng);
    let new_commitment = commit(new_salt, &segments);
    let proof = create_random_proof(
        ShotCircuit {
            old_commitment: Some(c0),
            new_commitment: Some(new_commitment),
            x: Some(shot.x),
            y: Some(shot.y),
            hit: Some(false),
            alive: Some(true),
            witness: Some(ShotWitness { old_salt: salt, new_salt, segments: segments.clone() }),
        },
        &sys().shot_params,
        &mut OsRng,
    )
    .unwrap();
    let mut bytes = Vec::new();
    proof.write(&mut bytes).unwrap();

    let report = fleetchain::game::HitReport {
        player_id: "cheat".into(),
        shot_index: shot.index,
        shot_x: shot.x,
        shot_y: shot.y,
        is_hit: false,
        is_alive: true,
        new_commitment: scalar_to_hex(&new_commitment),
        proof: bytes,
    };
    assert!(c.report_hit(report).is_err());
    assert_eq!(c.players["cheat"].commitment, c0, "state must not move");
    assert_eq!(c.players["cheat"].hits_taken, 0);
}

#[test]
fn players_joining_later_do_not_owe_earlier_shots() {
    let mut c = game();
    register(&mut c, "a", &far_fleet()).unwrap();
    register_default(&mut c, "b");
    c.fire_shot("a".into(), 0, 0).unwrap();
    register(&mut c, "late", &horizontal_fleet()).unwrap();
    assert!(c.pending_shots("late").is_empty());
    assert_eq!(c.pending_shots("b").len(), 1);

    ensure_shots(&mut c, "a", 1);
    c.fire_shot("a".into(), 1, 1).unwrap();
    assert_eq!(c.pending_shots("late").len(), 1, "but they owe later ones");
}

#[test]
fn a_fleet_can_be_sunk_cell_by_cell_and_is_then_out_of_the_game() {
    let mut c = game();
    register(&mut c, "a", &far_fleet()).unwrap();
    let mut b = register_default(&mut c, "b");
    let mut d = register(&mut c, "d", &horizontal_fleet()).unwrap();
    ensure_shots(&mut c, "a", 10);

    let targets = cells(&valid_fleet());
    for (i, &(x, y)) in targets.iter().enumerate() {
        c.fire_shot("a".into(), x, y).unwrap();
        let (hit, alive) = answer_next(&mut c, &mut b, "b");
        assert!(hit, "({x},{y})");
        assert_eq!(alive, i + 1 < targets.len(), "alive flag after hit #{}", i + 1);
        answer_next(&mut c, &mut d, "d"); // d answers each shot too
    }

    assert!(b.is_defeated());
    assert!(c.is_player_defeated("b"));
    assert!(!c.players["b"].forfeited, "defeated by shots, not by timeout");
    assert_eq!(c.get_active_players().len(), 2);
    assert!(c.mine_for_shots("b").unwrap_err().contains("Defeated"));
    assert!(c.fire_shot("b".into(), 0, 0).unwrap_err().contains("Defeated"));

    // A sunk player owes nothing, and cannot respond.
    ensure_shots(&mut c, "a", 1);
    c.fire_shot("a".into(), 5, 5).unwrap();
    assert!(c.pending_shots("b").is_empty());
    assert_eq!(c.pending_shots("d").len(), 1);
    let shot = c.shot_log.last().unwrap().clone();
    let late = b.respond(sys(), "b", &shot).unwrap();
    assert!(c.report_hit(late.report).is_err());
}

// ------------------------------------------------------------- deadlines ---

#[test]
fn silent_defenders_forfeit_when_a_deadline_is_set() {
    let mut c = game();
    c.response_timeout_blocks = Some(2);
    register(&mut c, "a", &far_fleet()).unwrap();
    let quiet = register_default(&mut c, "quiet");
    let mut prompt = register(&mut c, "prompt", &horizontal_fleet()).unwrap();

    c.fire_shot("a".into(), 0, 1).unwrap();
    answer_next(&mut c, &mut prompt, "prompt");

    // Chain grows but `quiet` never answers.
    c.mine_for_shots("a").unwrap();
    c.mine_for_shots("a").unwrap();
    assert!(!c.is_player_defeated("quiet"), "still within the deadline");
    c.mine_for_shots("a").unwrap();
    assert!(c.is_player_defeated("quiet"));
    assert!(!c.is_player_defeated("prompt"));
    assert!(!c.get_active_players().contains(&"quiet".to_string()));

    // The forfeit is enforced on the next state-changing call.
    assert!(c.fire_shot("quiet".into(), 1, 1).unwrap_err().contains("Defeated"));
    assert!(c.players["quiet"].forfeited);
    // A late proof does not resurrect them.
    let shot = c.shot_log[0].clone();
    let late = quiet.respond(sys(), "quiet", &shot).unwrap();
    assert!(c.report_hit(late.report).is_err());
}

#[test]
fn no_deadline_by_default() {
    let mut c = game();
    register(&mut c, "a", &far_fleet()).unwrap();
    register_default(&mut c, "quiet");
    c.fire_shot("a".into(), 0, 1).unwrap();
    for _ in 0..8 {
        c.mine_for_shots("a").unwrap();
    }
    assert!(!c.is_player_defeated("quiet"));
}

// ------------------------------------------------------------ whole game ---

#[test]
fn three_player_game_runs_to_a_single_survivor() {
    let mut c = game();
    let mut fleets = vec![
        ("p1".to_string(), register(&mut c, "p1", &valid_fleet()).unwrap(), valid_fleet()),
        ("p2".to_string(), register(&mut c, "p2", &horizontal_fleet()).unwrap(), horizontal_fleet()),
        ("p3".to_string(), register(&mut c, "p3", &far_fleet()).unwrap(), far_fleet()),
    ];
    // p3 hunts everyone down: it knows (in this test) both fleets.
    let mut targets: Vec<(u8, u8)> = cells(&fleets[0].2);
    targets.extend(cells(&fleets[1].2));
    targets.sort();
    targets.dedup(); // shared cells only need one shot: stacked ships across players
    ensure_shots(&mut c, "p3", targets.len());

    for (x, y) in targets {
        c.fire_shot("p3".into(), x, y).unwrap();
        for (id, secret, _) in fleets.iter_mut() {
            if id != "p3" && !c.pending_shots(id).is_empty() {
                answer_next(&mut c, secret, id);
            }
        }
    }
    assert!(c.is_player_defeated("p1"));
    assert!(c.is_player_defeated("p2"));
    assert_eq!(c.get_active_players(), vec!["p3".to_string()]);
    assert!(c.verify_blockchain());
}
