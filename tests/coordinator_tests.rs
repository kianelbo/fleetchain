use fleetchain::coordinator::GameCoordinator;
use fleetchain::game::Ship;
use fleetchain::crypto::{generate_salt, create_commitment};

// Helper function to create a valid 4-ship fleet
fn create_valid_fleet() -> Vec<Ship> {
    vec![
        Ship::new("Carrier".to_string(), vec![(0, 0), (0, 1), (0, 2), (0, 3)]),
        Ship::new("Cruiser".to_string(), vec![(2, 0), (2, 1), (2, 2)]),
        Ship::new("Submarine".to_string(), vec![(4, 0), (4, 1)]),
        Ship::new("Destroyer".to_string(), vec![(6, 0)]),
    ]
}

#[test]
fn test_coordinator_creation() {
    let coordinator = GameCoordinator::new(10, 2);
    assert_eq!(coordinator.grid.size, 10);
    assert_eq!(coordinator.blockchain.difficulty, 2);
    assert_eq!(coordinator.players.len(), 0);
    assert_eq!(coordinator.round, 0);
}

#[test]
fn test_register_player() {
    let mut coordinator = GameCoordinator::new(10, 2);
    
    let ships = create_valid_fleet();
    let positions: Vec<(u8, u8)> = ships.iter()
        .flat_map(|s| s.positions.clone())
        .collect();
    let salt = generate_salt();
    let commitment = create_commitment(&positions, &salt);
    
    let result = coordinator.register_player(
        "player1".to_string(),
        ships,
        commitment,
        salt,
    );
    
    assert!(result.is_ok());
    assert_eq!(coordinator.players.len(), 1);
}

#[test]
fn test_register_player_invalid_commitment() {
    let mut coordinator = GameCoordinator::new(10, 2);
    
    let ships = create_valid_fleet();
    let salt = generate_salt();
    let wrong_commitment = "wrong_commitment_hash".to_string();
    
    let result = coordinator.register_player(
        "player1".to_string(),
        ships,
        wrong_commitment,
        salt,
    );
    
    assert!(result.is_err());
    assert_eq!(coordinator.players.len(), 0);
}

#[test]
fn test_register_multiple_players() {
    let mut coordinator = GameCoordinator::new(10, 2);
    
    for i in 0..3 {
        let ships = vec![
            Ship::new(format!("Carrier{}", i), vec![(i, 0), (i, 1), (i, 2), (i, 3)]),
            Ship::new(format!("Cruiser{}", i), vec![(i, 4), (i, 5), (i, 6)]),
            Ship::new(format!("Submarine{}", i), vec![(i, 7), (i, 8)]),
            Ship::new(format!("Destroyer{}", i), vec![(i, 9)]),
        ];
        let positions: Vec<(u8, u8)> = ships.iter()
            .flat_map(|s| s.positions.clone())
            .collect();
        let salt = generate_salt();
        let commitment = create_commitment(&positions, &salt);
        
        coordinator.register_player(
            format!("player{}", i),
            ships,
            commitment,
            salt,
        ).unwrap();
    }
    
    assert_eq!(coordinator.players.len(), 3);
}

#[test]
fn test_mine_for_shots() {
    let mut coordinator = GameCoordinator::new(10, 2);
    
    let ships = create_valid_fleet();
    let positions: Vec<(u8, u8)> = ships.iter()
        .flat_map(|s| s.positions.clone())
        .collect();
    let salt = generate_salt();
    let commitment = create_commitment(&positions, &salt);
    
    coordinator.register_player(
        "player1".to_string(),
        ships,
        commitment,
        salt,
    ).unwrap();
    
    let shots = coordinator.mine_for_shots("player1").unwrap();
    assert!(shots > 0);
    
    let player = coordinator.players.get("player1").unwrap();
    assert_eq!(player.shots_available, shots);
}

#[test]
fn test_mine_for_nonexistent_player() {
    let mut coordinator = GameCoordinator::new(10, 2);
    let result = coordinator.mine_for_shots("nonexistent");
    assert!(result.is_err());
}

#[test]
fn test_fire_shot() {
    let mut coordinator = GameCoordinator::new(10, 2);
    
    let ships = create_valid_fleet();
    let positions: Vec<(u8, u8)> = ships.iter()
        .flat_map(|s| s.positions.clone())
        .collect();
    let salt = generate_salt();
    let commitment = create_commitment(&positions, &salt);
    
    coordinator.register_player(
        "player1".to_string(),
        ships,
        commitment,
        salt,
    ).unwrap();
    
    // Mine to get shots
    coordinator.mine_for_shots("player1").unwrap();
    
    // Fire shot
    let result = coordinator.fire_shot("player1".to_string(), 5, 5);
    assert!(result.is_ok());
    
    // Check transaction was added
    assert_eq!(coordinator.blockchain.pending_transactions.len(), 1);
}

#[test]
fn test_fire_shot_without_shots() {
    let mut coordinator = GameCoordinator::new(10, 2);
    
    let ships = create_valid_fleet();
    let positions: Vec<(u8, u8)> = ships.iter()
        .flat_map(|s| s.positions.clone())
        .collect();
    let salt = generate_salt();
    let commitment = create_commitment(&positions, &salt);
    
    coordinator.register_player(
        "player1".to_string(),
        ships,
        commitment,
        salt,
    ).unwrap();
    
    // Try to fire without mining
    let result = coordinator.fire_shot("player1".to_string(), 5, 5);
    assert!(result.is_err());
}

#[test]
fn test_get_active_players() {
    let mut coordinator = GameCoordinator::new(10, 2);
    
    for i in 0..3 {
        let ships = vec![
            Ship::new(format!("Carrier{}", i), vec![(i, 0), (i, 1), (i, 2), (i, 3)]),
            Ship::new(format!("Cruiser{}", i), vec![(i, 4), (i, 5), (i, 6)]),
            Ship::new(format!("Submarine{}", i), vec![(i, 7), (i, 8)]),
            Ship::new(format!("Destroyer{}", i), vec![(i, 9)]),
        ];
        let positions: Vec<(u8, u8)> = ships.iter()
            .flat_map(|s| s.positions.clone())
            .collect();
        let salt = generate_salt();
        let commitment = create_commitment(&positions, &salt);
        
        coordinator.register_player(
            format!("player{}", i),
            ships,
            commitment,
            salt,
        ).unwrap();
    }
    
    let active = coordinator.get_active_players();
    assert_eq!(active.len(), 3);
}

#[test]
fn test_verify_blockchain() {
    let mut coordinator = GameCoordinator::new(10, 2);
    
    let ships = create_valid_fleet();
    let positions: Vec<(u8, u8)> = ships.iter()
        .flat_map(|s| s.positions.clone())
        .collect();
    let salt = generate_salt();
    let commitment = create_commitment(&positions, &salt);
    
    coordinator.register_player("player1".to_string(), ships, commitment, salt).unwrap();
    coordinator.mine_for_shots("player1").unwrap();
    
    assert!(coordinator.verify_blockchain());
}

#[test]
fn test_game_stats() {
    let mut coordinator = GameCoordinator::new(10, 2);
    
    let ships = create_valid_fleet();
    let positions: Vec<(u8, u8)> = ships.iter()
        .flat_map(|s| s.positions.clone())
        .collect();
    let salt = generate_salt();
    let commitment = create_commitment(&positions, &salt);
    
    coordinator.register_player("player1".to_string(), ships, commitment, salt).unwrap();
    coordinator.mine_for_shots("player1").unwrap();
    
    let stats = coordinator.get_stats();
    assert_eq!(stats.total_players, 1);
    assert_eq!(stats.active_players, 1);
    assert!(stats.blockchain_length > 0);
}

#[test]
fn test_multiple_shots_and_mining() {
    let mut coordinator = GameCoordinator::new(10, 2);
    
    let ships = create_valid_fleet();
    let positions: Vec<(u8, u8)> = ships.iter()
        .flat_map(|s| s.positions.clone())
        .collect();
    let salt = generate_salt();
    let commitment = create_commitment(&positions, &salt);
    
    coordinator.register_player("player1".to_string(), ships, commitment, salt).unwrap();
    
    // Mine and shoot multiple times
    for i in 0..5 {
        coordinator.mine_for_shots("player1").unwrap();
        coordinator.fire_shot("player1".to_string(), i, i).unwrap();
        coordinator.mine_for_shots("player1").unwrap(); // Mine to include the shot
    }
    
    let stats = coordinator.get_stats();
    assert_eq!(stats.total_shots, 5);
}

#[test]
fn test_overlapping_ships_different_players() {
    let mut coordinator = GameCoordinator::new(10, 2);
    
    // Player 1
    let ships1 = vec![
        Ship::new("Carrier1".to_string(), vec![(0, 0), (0, 1), (0, 2), (0, 3)]),
        Ship::new("Cruiser1".to_string(), vec![(2, 0), (2, 1), (2, 2)]),
        Ship::new("Submarine1".to_string(), vec![(4, 0), (4, 1)]),
        Ship::new("Destroyer1".to_string(), vec![(6, 0)]),
    ];
    let positions1: Vec<(u8, u8)> = ships1.iter()
        .flat_map(|s| s.positions.clone())
        .collect();
    let salt1 = generate_salt();
    let commitment1 = create_commitment(&positions1, &salt1);
    
    // Player 2 with overlapping position
    let ships2 = vec![
        Ship::new("Carrier2".to_string(), vec![(0, 0), (1, 0), (2, 0), (3, 0)]),
        Ship::new("Cruiser2".to_string(), vec![(0, 2), (1, 2), (2, 2)]),
        Ship::new("Submarine2".to_string(), vec![(0, 4), (1, 4)]),
        Ship::new("Destroyer2".to_string(), vec![(0, 6)]),
    ];
    let positions2: Vec<(u8, u8)> = ships2.iter()
        .flat_map(|s| s.positions.clone())
        .collect();
    let salt2 = generate_salt();
    let commitment2 = create_commitment(&positions2, &salt2);
    
    coordinator.register_player("player1".to_string(), ships1, commitment1, salt1).unwrap();
    coordinator.register_player("player2".to_string(), ships2, commitment2, salt2).unwrap();
    
    assert_eq!(coordinator.players.len(), 2);
}

#[test]
fn test_blockchain_grows_with_mining() {
    let mut coordinator = GameCoordinator::new(10, 2);
    
    let ships = create_valid_fleet();
    let positions: Vec<(u8, u8)> = ships.iter()
        .flat_map(|s| s.positions.clone())
        .collect();
    let salt = generate_salt();
    let commitment = create_commitment(&positions, &salt);
    
    coordinator.register_player("player1".to_string(), ships, commitment, salt).unwrap();
    
    let initial_length = coordinator.blockchain.chain.len();
    
    for _ in 0..3 {
        coordinator.mine_for_shots("player1").unwrap();
    }
    
    assert_eq!(coordinator.blockchain.chain.len(), initial_length + 3);
}

#[test]
fn test_pending_transactions_cleared_after_mining() {
    let mut coordinator = GameCoordinator::new(10, 2);
    
    let ships = create_valid_fleet();
    let positions: Vec<(u8, u8)> = ships.iter()
        .flat_map(|s| s.positions.clone())
        .collect();
    let salt = generate_salt();
    let commitment = create_commitment(&positions, &salt);
    
    coordinator.register_player("player1".to_string(), ships, commitment, salt).unwrap();
    
    // Mine to get initial shots
    coordinator.mine_for_shots("player1").unwrap();
    coordinator.mine_for_shots("player1").unwrap();
    coordinator.mine_for_shots("player1").unwrap();
    
    // Fire multiple shots
    for i in 0..3 {
        coordinator.fire_shot("player1".to_string(), i, i).unwrap();
    }
    
    assert_eq!(coordinator.blockchain.pending_transactions.len(), 3);
    
    // Mine to clear pending
    coordinator.mine_for_shots("player1").unwrap();
    assert_eq!(coordinator.blockchain.pending_transactions.len(), 0);
}

#[test]
fn test_stats_serialization() {
    let mut coordinator = GameCoordinator::new(10, 2);
    
    let ships = create_valid_fleet();
    let positions: Vec<(u8, u8)> = ships.iter()
        .flat_map(|s| s.positions.clone())
        .collect();
    let salt = generate_salt();
    let commitment = create_commitment(&positions, &salt);
    
    coordinator.register_player("player1".to_string(), ships, commitment, salt).unwrap();
    
    let stats = coordinator.get_stats();
    let json = serde_json::to_string(&stats).unwrap();
    let deserialized: fleetchain::coordinator::GameStats = serde_json::from_str(&json).unwrap();
    
    assert_eq!(stats.total_players, deserialized.total_players);
    assert_eq!(stats.blockchain_length, deserialized.blockchain_length);
}

#[test]
fn test_concurrent_players_mining() {
    let mut coordinator = GameCoordinator::new(10, 2);
    
    // Register two players
    for i in 0..2 {
        let ships = vec![
            Ship::new(format!("Carrier{}", i), vec![(i, 0), (i, 1), (i, 2), (i, 3)]),
            Ship::new(format!("Cruiser{}", i), vec![(i, 4), (i, 5), (i, 6)]),
            Ship::new(format!("Submarine{}", i), vec![(i, 7), (i, 8)]),
            Ship::new(format!("Destroyer{}", i), vec![(i, 9)]),
        ];
        let positions: Vec<(u8, u8)> = ships.iter()
            .flat_map(|s| s.positions.clone())
            .collect();
        let salt = generate_salt();
        let commitment = create_commitment(&positions, &salt);
        
        coordinator.register_player(
            format!("player{}", i),
            ships,
            commitment,
            salt,
        ).unwrap();
    }
    
    // Both players mine
    coordinator.mine_for_shots("player0").unwrap();
    coordinator.mine_for_shots("player1").unwrap();
    
    // Both should have shots
    assert!(coordinator.players.get("player0").unwrap().shots_available > 0);
    assert!(coordinator.players.get("player1").unwrap().shots_available > 0);
}

#[test]
fn test_fleet_validation_correct_fleet() {
    let mut coordinator = GameCoordinator::new(10, 2);
    let ships = create_valid_fleet();
    let positions: Vec<(u8, u8)> = ships.iter()
        .flat_map(|s| s.positions.clone())
        .collect();
    let salt = generate_salt();
    let commitment = create_commitment(&positions, &salt);

    let result = coordinator.register_player(
        "player1".to_string(),
        ships,
        commitment,
        salt,
    );

    assert!(result.is_ok());
}

#[test]
fn test_fleet_validation_wrong_number_of_ships() {
    let mut coordinator = GameCoordinator::new(10, 2);
    
    // Only 3 ships instead of 4
    let ships = vec![
        Ship::new("Carrier".to_string(), vec![(0, 0), (0, 1), (0, 2), (0, 3)]),
        Ship::new("Cruiser".to_string(), vec![(2, 0), (2, 1), (2, 2)]),
        Ship::new("Submarine".to_string(), vec![(4, 0), (4, 1)]),
    ];
    let positions: Vec<(u8, u8)> = ships.iter()
        .flat_map(|s| s.positions.clone())
        .collect();
    let salt = generate_salt();
    let commitment = create_commitment(&positions, &salt);

    let result = coordinator.register_player(
        "player1".to_string(),
        ships,
        commitment,
        salt,
    );

    assert!(result.is_err());
    assert!(result.unwrap_err().contains("exactly 4 ships"));
}

#[test]
fn test_fleet_validation_wrong_ship_sizes() {
    let mut coordinator = GameCoordinator::new(10, 2);
    
    // Wrong sizes: two 2-cell ships instead of 1,2,3,4
    let ships = vec![
        Ship::new("Ship1".to_string(), vec![(0, 0), (0, 1), (0, 2), (0, 3)]),
        Ship::new("Ship2".to_string(), vec![(2, 0), (2, 1), (2, 2)]),
        Ship::new("Ship3".to_string(), vec![(4, 0), (4, 1)]),
        Ship::new("Ship4".to_string(), vec![(6, 0), (6, 1)]), // Should be 1 cell
    ];
    let positions: Vec<(u8, u8)> = ships.iter()
        .flat_map(|s| s.positions.clone())
        .collect();
    let salt = generate_salt();
    let commitment = create_commitment(&positions, &salt);

    let result = coordinator.register_player(
        "player1".to_string(),
        ships,
        commitment,
        salt,
    );

    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Carrier"));
}

#[test]
fn test_fleet_validation_diagonal_placement() {
    let mut coordinator = GameCoordinator::new(10, 2);
    
    // Diagonal ship - invalid
    let ships = vec![
        Ship::new("Carrier".to_string(), vec![(0, 0), (1, 1), (2, 2), (3, 3)]), // Diagonal!
        Ship::new("Cruiser".to_string(), vec![(2, 0), (2, 1), (2, 2)]),
        Ship::new("Submarine".to_string(), vec![(4, 0), (4, 1)]),
        Ship::new("Destroyer".to_string(), vec![(6, 0)]),
    ];
    let positions: Vec<(u8, u8)> = ships.iter()
        .flat_map(|s| s.positions.clone())
        .collect();
    let salt = generate_salt();
    let commitment = create_commitment(&positions, &salt);

    let result = coordinator.register_player(
        "player1".to_string(),
        ships,
        commitment,
        salt,
    );

    assert!(result.is_err());
    assert!(result.unwrap_err().contains("horizontally or vertically"));
}

#[test]
fn test_fleet_validation_non_consecutive_placement() {
    let mut coordinator = GameCoordinator::new(10, 2);
    
    // Non-consecutive horizontal placement
    let ships = vec![
        Ship::new("Carrier".to_string(), vec![(0, 0), (0, 1), (0, 3), (0, 4)]), // Gap at (0,2)
        Ship::new("Cruiser".to_string(), vec![(2, 0), (2, 1), (2, 2)]),
        Ship::new("Submarine".to_string(), vec![(4, 0), (4, 1)]),
        Ship::new("Destroyer".to_string(), vec![(6, 0)]),
    ];
    let positions: Vec<(u8, u8)> = ships.iter()
        .flat_map(|s| s.positions.clone())
        .collect();
    let salt = generate_salt();
    let commitment = create_commitment(&positions, &salt);

    let result = coordinator.register_player(
        "player1".to_string(),
        ships,
        commitment,
        salt,
    );

    assert!(result.is_err());
    assert!(result.unwrap_err().contains("horizontally or vertically"));
}

#[test]
fn test_fleet_validation_vertical_placement() {
    let mut coordinator = GameCoordinator::new(10, 2);
    
    // All ships placed vertically - should be valid
    let ships = vec![
        Ship::new("Carrier".to_string(), vec![(0, 0), (1, 0), (2, 0), (3, 0)]),
        Ship::new("Cruiser".to_string(), vec![(0, 2), (1, 2), (2, 2)]),
        Ship::new("Submarine".to_string(), vec![(0, 4), (1, 4)]),
        Ship::new("Destroyer".to_string(), vec![(0, 6)]),
    ];
    let positions: Vec<(u8, u8)> = ships.iter()
        .flat_map(|s| s.positions.clone())
        .collect();
    let salt = generate_salt();
    let commitment = create_commitment(&positions, &salt);

    let result = coordinator.register_player(
        "player1".to_string(),
        ships,
        commitment,
        salt,
    );

    assert!(result.is_ok());
}

#[test]
fn test_defeated_player_cannot_mine() {
    let mut coordinator = GameCoordinator::new(10, 2);
    
    // Register player with small fleet for easy defeat
    let ships = vec![
        Ship::new("Carrier".to_string(), vec![(0, 0), (0, 1), (0, 2), (0, 3)]),
        Ship::new("Cruiser".to_string(), vec![(2, 0), (2, 1), (2, 2)]),
        Ship::new("Submarine".to_string(), vec![(4, 0), (4, 1)]),
        Ship::new("Destroyer".to_string(), vec![(6, 0)]),
    ];
    let positions: Vec<(u8, u8)> = ships.iter()
        .flat_map(|s| s.positions.clone())
        .collect();
    let salt = generate_salt();
    let commitment = create_commitment(&positions, &salt);

    coordinator.register_player(
        "player1".to_string(),
        ships,
        commitment,
        salt,
    ).unwrap();

    // Player can mine initially
    assert!(coordinator.mine_for_shots("player1").is_ok());

    // Sink all ships
    if let Some(player) = coordinator.players.get_mut("player1") {
        for ship in &mut player.ships {
            for i in 0..ship.positions.len() {
                ship.hits[i] = true;
            }
        }
    }

    // Verify player is defeated
    assert!(coordinator.is_player_defeated("player1"));

    // Defeated player cannot mine
    let result = coordinator.mine_for_shots("player1");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Defeated players cannot mine"));
}

#[test]
fn test_large_scale_game() {
    let mut coordinator = GameCoordinator::new(20, 2);
    
    // Register 10 players
    for i in 0..10 {
        let ships = vec![
            Ship::new(format!("Carrier{}", i), vec![(i, 0), (i, 1), (i, 2), (i, 3)]),
            Ship::new(format!("Cruiser{}", i), vec![(i, 4), (i, 5), (i, 6)]),
            Ship::new(format!("Submarine{}", i), vec![(i, 7), (i, 8)]),
            Ship::new(format!("Destroyer{}", i), vec![(i, 9)]),
        ];
        let positions: Vec<(u8, u8)> = ships.iter()
            .flat_map(|s| s.positions.clone())
            .collect();
        let salt = generate_salt();
        let commitment = create_commitment(&positions, &salt);
        
        coordinator.register_player(
            format!("player{}", i),
            ships,
            commitment,
            salt,
        ).unwrap();
    }
    
    assert_eq!(coordinator.players.len(), 10);
    
    // Each player mines and shoots
    for i in 0..10 {
        coordinator.mine_for_shots(&format!("player{}", i)).unwrap();
        coordinator.fire_shot(format!("player{}", i), i, i).unwrap();
        coordinator.mine_for_shots(&format!("player{}", i)).unwrap(); // Mine to include shot
    }
    
    let stats = coordinator.get_stats();
    assert_eq!(stats.total_players, 10);
    assert_eq!(stats.total_shots, 10);
}
