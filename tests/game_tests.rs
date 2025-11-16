use fleetchain::game::{Ship, Grid, Player};
use fleetchain::crypto::{generate_salt, create_commitment};

#[test]
fn test_ship_creation() {
    let ship = Ship::new("carrier".to_string(), vec![(0, 0), (0, 1), (0, 2)]);
    assert_eq!(ship.id, "carrier");
    assert_eq!(ship.positions.len(), 3);
    assert_eq!(ship.hits.len(), 3);
    assert!(ship.hits.iter().all(|&h| !h));
}

#[test]
fn test_ship_hit_detection() {
    let mut ship = Ship::new("carrier".to_string(), vec![(0, 0), (0, 1), (0, 2)]);
    
    assert!(ship.register_hit(0, 1));
    assert!(!ship.register_hit(5, 5)); // Miss
    assert!(ship.hits[1]); // Second position should be hit
}

#[test]
fn test_ship_sinking() {
    let mut ship = Ship::new("destroyer".to_string(), vec![(0, 0), (0, 1)]);
    
    assert!(!ship.is_sunk());
    ship.register_hit(0, 0);
    assert!(!ship.is_sunk());
    ship.register_hit(0, 1);
    assert!(ship.is_sunk());
}

#[test]
fn test_ship_multiple_hits_same_position() {
    let mut ship = Ship::new("carrier".to_string(), vec![(0, 0), (0, 1)]);
    
    assert!(ship.register_hit(0, 0));
    assert!(ship.register_hit(0, 0)); // Hit same position again
    assert!(ship.hits[0]);
}

#[test]
fn test_grid_creation() {
    let grid = Grid::new(10);
    assert_eq!(grid.size, 10);
    assert_eq!(grid.cells.len(), 0);
}

#[test]
fn test_grid_place_ship() {
    let mut grid = Grid::new(10);
    let positions = vec![(0, 0), (0, 1), (0, 2)];
    
    let result = grid.place_ship("player1", &positions);
    assert!(result.is_ok());
}

#[test]
fn test_grid_out_of_bounds() {
    let mut grid = Grid::new(10);
    let positions = vec![(10, 10), (11, 11)]; // Out of bounds
    
    let result = grid.place_ship("player1", &positions);
    assert!(result.is_err());
}

#[test]
fn test_grid_multiple_ships_same_cell() {
    let mut grid = Grid::new(10);
    
    grid.place_ship("player1", &vec![(0, 0), (0, 1)]).unwrap();
    grid.place_ship("player2", &vec![(0, 0), (1, 0)]).unwrap();
    
    // Both players should have ships at (0, 0)
    let players = grid.cells.get(&(0, 0)).unwrap();
    assert_eq!(players.len(), 2);
    assert!(players.contains(&"player1".to_string()));
    assert!(players.contains(&"player2".to_string()));
}

#[test]
fn test_grid_boundary_positions() {
    let mut grid = Grid::new(10);
    
    // Test corners
    assert!(grid.place_ship("p1", &vec![(0, 0)]).is_ok());
    assert!(grid.place_ship("p2", &vec![(9, 9)]).is_ok());
    assert!(grid.place_ship("p3", &vec![(0, 9)]).is_ok());
    assert!(grid.place_ship("p4", &vec![(9, 0)]).is_ok());
}

#[test]
fn test_player_creation() {
    let ships = vec![
        Ship::new("carrier".to_string(), vec![(0, 0), (0, 1), (0, 2)]),
    ];
    let salt = generate_salt();
    let positions: Vec<(u8, u8)> = ships.iter()
        .flat_map(|s| s.positions.clone())
        .collect();
    let commitment = create_commitment(&positions, &salt);

    let player = Player::new("player1".to_string(), ships, commitment.clone(), salt.clone());

    assert_eq!(player.id, "player1");
    assert_eq!(player.ships.len(), 1);
    assert_eq!(player.board_commitment, commitment);
}

#[test]
fn test_player_check_hit() {
    let ships = vec![
        Ship::new("carrier".to_string(), vec![(0, 0), (0, 1), (0, 2)]),
        Ship::new("destroyer".to_string(), vec![(5, 5), (5, 6)]),
    ];
    let salt = generate_salt();
    let positions: Vec<(u8, u8)> = ships.iter()
        .flat_map(|s| s.positions.clone())
        .collect();
    let commitment = create_commitment(&positions, &salt);
    
    let mut player = Player::new("player1".to_string(), ships, commitment, salt);
    
    assert!(player.check_hit(0, 1)); // Hit on carrier
    assert!(player.check_hit(5, 5)); // Hit on destroyer
    assert!(!player.check_hit(9, 9)); // Miss
}

#[test]
fn test_player_defeat() {
    let ships = vec![
        Ship::new("small".to_string(), vec![(0, 0), (0, 1)]),
    ];
    let salt = generate_salt();
    let positions: Vec<(u8, u8)> = ships.iter()
        .flat_map(|s| s.positions.clone())
        .collect();
    let commitment = create_commitment(&positions, &salt);
    
    let mut player = Player::new("player1".to_string(), ships, commitment, salt);
    
    assert!(!player.is_defeated());
    player.check_hit(0, 0);
    assert!(!player.is_defeated());
    player.check_hit(0, 1);
    assert!(player.is_defeated());
}

#[test]
fn test_player_get_all_positions() {
    let ships = vec![
        Ship::new("ship1".to_string(), vec![(0, 0), (0, 1)]),
        Ship::new("ship2".to_string(), vec![(5, 5)]),
    ];
    let salt = generate_salt();
    let positions: Vec<(u8, u8)> = ships.iter()
        .flat_map(|s| s.positions.clone())
        .collect();
    let commitment = create_commitment(&positions, &salt);
    
    let player = Player::new("player1".to_string(), ships, commitment, salt);
    let all_positions = player.get_all_ship_positions();
    
    assert_eq!(all_positions.len(), 3);
    assert!(all_positions.contains(&(0, 0)));
    assert!(all_positions.contains(&(0, 1)));
    assert!(all_positions.contains(&(5, 5)));
}

#[test]
fn test_multiple_players_on_grid() {
    let mut grid = Grid::new(10);
    
    // Player 1 ships
    grid.place_ship("player1", &vec![(0, 0), (0, 1), (0, 2)]).unwrap();
    
    // Player 2 ships (overlapping at (0, 1))
    grid.place_ship("player2", &vec![(0, 1), (1, 1), (2, 1)]).unwrap();
    
    // Check overlapping cell
    let players_at_0_1 = grid.cells.get(&(0, 1)).unwrap();
    assert_eq!(players_at_0_1.len(), 2);
}

#[test]
fn test_ship_serialization() {
    let ship = Ship::new("carrier".to_string(), vec![(0, 0), (0, 1)]);
    let json = serde_json::to_string(&ship).unwrap();
    let deserialized: Ship = serde_json::from_str(&json).unwrap();
    
    assert_eq!(ship.id, deserialized.id);
    assert_eq!(ship.positions, deserialized.positions);
    assert_eq!(ship.hits, deserialized.hits);
}

#[test]
fn test_large_grid() {
    let mut grid = Grid::new(100);
    
    // Place ships across large grid
    for i in 0..10 {
        let positions = vec![(i * 10, i * 10), (i * 10 + 1, i * 10)];
        assert!(grid.place_ship(&format!("player{}", i), &positions).is_ok());
    }
}

#[test]
fn test_ship_with_single_cell() {
    let ship = Ship::new("submarine".to_string(), vec![(5, 5)]);
    assert_eq!(ship.positions.len(), 1);
    assert!(!ship.is_sunk());
}

