mod blockchain;
mod game;
mod crypto;
mod coordinator;

use coordinator::GameCoordinator;
use game::Ship;
use crypto::{generate_salt, create_commitment};

fn main() {
    println!("=== FleetChain: Blockchain Battleship ===\n");

    // Initialize game
    let grid_size = 10;
    let mining_difficulty = 2;
    let mut game = GameCoordinator::new(grid_size, mining_difficulty);

    println!("Game initialized with {}x{} grid", grid_size, grid_size);
    println!("Mining difficulty: {}\n", mining_difficulty);

    // Demo: Register two players
    demo_game(&mut game);
}

fn demo_game(game: &mut GameCoordinator) {
    // Player 1 setup
    println!("Registering Player 1...");
    let player1_ships = vec![
        Ship::new("carrier".to_string(), vec![(0, 0), (0, 1), (0, 2), (0, 3), (0, 4)]),
        Ship::new("battleship".to_string(), vec![(2, 0), (2, 1), (2, 2), (2, 3)]),
        Ship::new("destroyer".to_string(), vec![(4, 0), (4, 1), (4, 2)]),
    ];

    let player1_positions: Vec<(u8, u8)> = player1_ships.iter()
        .flat_map(|ship| ship.positions.clone())
        .collect();

    let player1_salt = generate_salt();
    let player1_commitment = create_commitment(&player1_positions, &player1_salt);

    match game.register_player(
        "player1".to_string(),
        player1_ships,
        player1_commitment.clone(),
        player1_salt.clone(),
    ) {
        Ok(_) => println!("✓ Player 1 registered with commitment: {}...", &player1_commitment[..16]),
        Err(e) => println!("✗ Failed to register Player 1: {}", e),
    }

    // Player 2 setup
    println!("\nRegistering Player 2...");
    let player2_ships = vec![
        Ship::new("carrier".to_string(), vec![(5, 5), (5, 6), (5, 7), (5, 8), (5, 9)]),
        Ship::new("battleship".to_string(), vec![(7, 5), (7, 6), (7, 7), (7, 8)]),
        Ship::new("destroyer".to_string(), vec![(9, 5), (9, 6), (9, 7)]),
    ];

    let player2_positions: Vec<(u8, u8)> = player2_ships.iter()
        .flat_map(|ship| ship.positions.clone())
        .collect();

    let player2_salt = generate_salt();
    let player2_commitment = create_commitment(&player2_positions, &player2_salt);

    match game.register_player(
        "player2".to_string(),
        player2_ships,
        player2_commitment.clone(),
        player2_salt.clone(),
    ) {
        Ok(_) => println!("✓ Player 2 registered with commitment: {}...", &player2_commitment[..16]),
        Err(e) => println!("✗ Failed to register Player 2: {}", e),
    }

    // Display initial stats
    println!("\n{}", game.get_stats());

    println!("\n--- Mining Phase ---");
    println!("Player 1 mining for shots...");
    match game.mine_for_shots("player1") {
        Ok(shots) => println!("✓ Player 1 earned {} shot(s)", shots),
        Err(e) => println!("✗ Mining failed: {}", e),
    }

    println!("Player 2 mining for shots...");
    match game.mine_for_shots("player2") {
        Ok(shots) => println!("✓ Player 2 earned {} shot(s)", shots),
        Err(e) => println!("✗ Mining failed: {}", e),
    }

    // Shooting demonstration
    println!("\n--- Combat Phase ---");
    println!("Player 1 fires at (5, 5)...");
    match game.fire_shot("player1".to_string(), 5, 5) {
        Ok(_) => println!("✓ Shot fired! Transaction added to blockchain"),
        Err(e) => println!("✗ Shot failed: {}", e),
    }

    println!("Player 2 fires at (0, 0)...");
    match game.fire_shot("player2".to_string(), 0, 0) {
        Ok(_) => println!("✓ Shot fired! Transaction added to blockchain"),
        Err(e) => println!("✗ Shot failed: {}", e),
    }

    // Mine the transactions
    println!("\nMining combat transactions...");
    game.mine_for_shots("player1").ok();

    // Display final stats
    println!("\n--- Final Stats ---");
    println!("{}", game.get_stats());
    println!("Blockchain valid: {}", game.verify_blockchain());
    
    // Display blockchain
    println!("\n--- Blockchain ---");
    for (i, block) in game.blockchain.chain.iter().enumerate() {
        println!("Block #{}: {} transactions, hash: {}...", 
            i, block.transactions.len(), &block.hash[..16]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_initialization() {
        let game = GameCoordinator::new(10, 2);
        assert_eq!(game.grid.size, 10);
        assert_eq!(game.blockchain.difficulty, 2);
    }
}
