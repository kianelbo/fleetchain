#![allow(dead_code)]
//! Shared helpers for integration tests.

use fleetchain::coordinator::GameCoordinator;
use fleetchain::game::Ship;
use fleetchain::zk::{FleetSecret, ZkSystem};

/// Development proving/verifying keys, generated once per test binary.
/// (This is also what `GameCoordinator` falls back to, so proofs made with it
/// verify there.)
pub fn sys() -> &'static ZkSystem {
    ZkSystem::insecure_dev_shared()
}

/// A valid 4-ship fleet: Carrier (0,0)-(0,3), Cruiser (2,0)-(2,2),
/// Submarine (4,0)-(4,1), Destroyer (6,0).
pub fn valid_fleet() -> Vec<Ship> {
    vec![
        Ship::new("Carrier".to_string(), vec![(0, 0), (0, 1), (0, 2), (0, 3)]),
        Ship::new("Cruiser".to_string(), vec![(2, 0), (2, 1), (2, 2)]),
        Ship::new("Submarine".to_string(), vec![(4, 0), (4, 1)]),
        Ship::new("Destroyer".to_string(), vec![(6, 0)]),
    ]
}

/// Same shape, but every ship is horizontal (varying x) on rows y = 0,2,4,6.
pub fn horizontal_fleet() -> Vec<Ship> {
    vec![
        Ship::new("Carrier".to_string(), vec![(0, 0), (1, 0), (2, 0), (3, 0)]),
        Ship::new("Cruiser".to_string(), vec![(0, 2), (1, 2), (2, 2)]),
        Ship::new("Submarine".to_string(), vec![(0, 4), (1, 4)]),
        Ship::new("Destroyer".to_string(), vec![(0, 6)]),
    ]
}

/// All cells occupied by a fleet.
pub fn cells(ships: &[Ship]) -> Vec<(u8, u8)> {
    ships.iter().flat_map(|s| s.positions.clone()).collect()
}

/// Build the secret, prove it legal, and register it with `coord`.
pub fn register(coord: &mut GameCoordinator, id: &str, ships: &[Ship]) -> Result<FleetSecret, String> {
    let secret = FleetSecret::from_ships(ships, coord.grid.size)?;
    let proof = secret.prove_setup(sys())?;
    coord.register_player(id.to_string(), secret.commitment_hex(), &proof)?;
    Ok(secret)
}

/// Register a player with the standard fleet.
pub fn register_default(coord: &mut GameCoordinator, id: &str) -> FleetSecret {
    register(coord, id, &valid_fleet()).expect("registration")
}

/// Have `id` answer its oldest pending shot; returns (hit, alive).
pub fn answer_next(coord: &mut GameCoordinator, secret: &mut FleetSecret, id: &str) -> (bool, bool) {
    let shot = coord.pending_shots(id).into_iter().next().expect("a pending shot");
    let pending = secret.respond(sys(), id, &shot).expect("proving");
    let out = coord.report_hit(pending.report.clone()).expect("coordinator accepts honest response");
    secret.apply(pending);
    (out.is_hit, !out.defeated)
}

/// Mine until `id` holds at least `n` unspent shots.
pub fn ensure_shots(coord: &mut GameCoordinator, id: &str, n: usize) {
    for _ in 0..200 {
        if coord.blockchain.get_unspent_shots(id) as usize >= n {
            return;
        }
        coord.mine_for_shots(id).expect("mine");
    }
    panic!("could not accumulate {n} shots");
}
