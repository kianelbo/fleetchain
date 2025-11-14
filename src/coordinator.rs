use crate::blockchain::{Blockchain, Transaction};
use crate::game::{Grid, Player, Ship, HitReport};
use crate::crypto::{verify_commitment, HitProof};
use std::collections::HashMap;
use std::path::PathBuf;

/// Coordinates the entire game including blockchain and game state
pub struct GameCoordinator {
    pub blockchain: Blockchain,
    pub grid: Grid,
    pub players: HashMap<String, Player>,
    pub round: u32,
    blockchain_path: Option<PathBuf>,
}

impl GameCoordinator {
    pub fn new(grid_size: u8, mining_difficulty: usize) -> Self {
        Self {
            blockchain: Blockchain::new(mining_difficulty),
            grid: Grid::new(grid_size),
            players: HashMap::new(),
            round: 0,
            blockchain_path: None,
        }
    }

    /// Create a new GameCoordinator with blockchain persistence
    pub fn with_persistence(grid_size: u8, mining_difficulty: usize, blockchain_path: PathBuf) -> Self {
        let blockchain = if Blockchain::file_exists(&blockchain_path) {
            println!("Loading existing blockchain from {:?}...", blockchain_path);
            match Blockchain::load_from_file(&blockchain_path) {
                Ok(bc) => {
                    println!("✓ Loaded blockchain with {} blocks", bc.chain.len());
                    bc
                }
                Err(e) => {
                    eprintln!("✗ Failed to load blockchain: {}", e);
                    eprintln!("  Creating new blockchain instead");
                    Blockchain::new(mining_difficulty)
                }
            }
        } else {
            println!("No existing blockchain found, creating new one");
            Blockchain::new(mining_difficulty)
        };

        let coordinator = Self {
            blockchain,
            grid: Grid::new(grid_size),
            players: HashMap::new(),
            round: 0,
            blockchain_path: Some(blockchain_path),
        };

        // Save the initial blockchain to disk
        if let Err(e) = coordinator.save_blockchain() {
            eprintln!("Warning: Failed to save initial blockchain: {}", e);
        }

        coordinator
    }

    /// Save the blockchain to disk if persistence is enabled
    fn save_blockchain(&self) -> Result<(), String> {
        if let Some(path) = &self.blockchain_path {
            self.blockchain.save_to_file(path)?;
        }
        Ok(())
    }

    /// Manually save the blockchain (public method for external use)
    pub fn save(&self) -> Result<(), String> {
        self.save_blockchain()
    }

    /// Register a new player with their fleet
    pub fn register_player(
        &mut self,
        player_id: String,
        ships: Vec<Ship>,
        board_commitment: String,
        salt: String,
    ) -> Result<(), String> {
        // Validate fleet composition: must have exactly 4 ships
        if ships.len() != 4 {
            return Err("Fleet must contain exactly 4 ships".to_string());
        }

        // Validate ship sizes and check for required ships
        let mut ship_sizes: Vec<usize> = ships.iter()
            .map(|ship| ship.positions.len())
            .collect();
        ship_sizes.sort_unstable();

        // Required: 1 Destroyer (1 cell), 1 Submarine (2 cells), 1 Cruiser (3 cells), 1 Carrier (4 cells)
        if ship_sizes != vec![1, 2, 3, 4] {
            return Err("Fleet must contain: 1 Carrier (4 cells), 1 Cruiser (3 cells), 1 Submarine (2 cells), 1 Destroyer (1 cell)".to_string());
        }

        // Validate ship placement (horizontal or vertical)
        for ship in &ships {
            if !Self::is_valid_ship_placement(&ship.positions) {
                return Err(format!("Ship '{}' must be placed horizontally or vertically in a continuous line", ship.id));
            }
        }

        // Verify the commitment matches the ships
        let all_positions: Vec<(u8, u8)> = ships.iter()
            .flat_map(|ship| ship.positions.clone())
            .collect();

        if !verify_commitment(&board_commitment, &all_positions, &salt) {
            return Err("Invalid board commitment".to_string());
        }

        // Place ships on the shared grid
        for ship in &ships {
            self.grid.place_ship(&player_id, &ship.positions)?;
        }

        // Create player
        let player = Player::new(player_id.clone(), ships, board_commitment, salt);
        self.players.insert(player_id, player);

        Ok(())
    }

    /// Validate that a ship is placed horizontally or vertically in a continuous line
    fn is_valid_ship_placement(positions: &[(u8, u8)]) -> bool {
        if positions.is_empty() {
            return false;
        }
        if positions.len() == 1 {
            return true; // Single cell is always valid
        }

        let mut sorted_positions = positions.to_vec();
        sorted_positions.sort_unstable();

        // Check if horizontal (same y, consecutive x)
        let all_same_y = sorted_positions.iter().all(|&(_, y)| y == sorted_positions[0].1);
        if all_same_y {
            for i in 1..sorted_positions.len() {
                if sorted_positions[i].0 != sorted_positions[i - 1].0 + 1 {
                    return false; // Not consecutive
                }
            }
            return true;
        }

        // Check if vertical (same x, consecutive y)
        let all_same_x = sorted_positions.iter().all(|&(x, _)| x == sorted_positions[0].0);
        if all_same_x {
            for i in 1..sorted_positions.len() {
                if sorted_positions[i].1 != sorted_positions[i - 1].1 + 1 {
                    return false; // Not consecutive
                }
            }
            return true;
        }

        false // Neither horizontal nor vertical
    }

    /// Allow a player to mine for shots
    pub fn mine_for_shots(&mut self, player_id: &str) -> Result<u32, String> {
        if !self.players.contains_key(player_id) {
            return Err("Player not found".to_string());
        }

        // Check if player is defeated
        if self.is_player_defeated(player_id) {
            return Err("Defeated players cannot mine".to_string());
        }

        // Mine pending transactions
        let shots_earned = self.blockchain.mine_pending_transactions(player_id);

        // Award shots to the miner
        if let Some(player) = self.players.get_mut(player_id) {
            player.add_shots(shots_earned);
        }

        // Auto-save blockchain after mining
        if let Err(e) = self.save_blockchain() {
            eprintln!("Warning: Failed to save blockchain: {}", e);
        }

        Ok(shots_earned)
    }

    /// Fire a shot (creates a transaction)
    pub fn fire_shot(
        &mut self,
        player_id: String,
        target_x: u8,
        target_y: u8,
    ) -> Result<(), String> {
        // Check if player has shots available
        let player = self.players.get_mut(&player_id)
            .ok_or("Player not found")?;

        player.fire_shot(target_x, target_y)?;

        // Create transaction
        let transaction = Transaction::new(player_id, target_x, target_y, 0);
        self.blockchain.add_transaction(transaction);

        // Auto-save blockchain after adding transaction
        if let Err(e) = self.save_blockchain() {
            eprintln!("Warning: Failed to save blockchain: {}", e);
        }

        Ok(())
    }

    /// Process hit reports with ZK proofs
    #[allow(dead_code)]
    pub fn report_hit(
        &mut self,
        report: HitReport,
    ) -> Result<bool, String> {
        let player = self.players.get(&report.player_id)
            .ok_or("Player not found")?;

        // Deserialize proof
        let proof: HitProof = serde_json::from_slice(&report.proof)
            .map_err(|_| "Invalid proof format")?;

        // Verify the proof
        let is_valid = if report.is_hit {
            proof.verify_hit((report.shot_x, report.shot_y), &player.board_commitment)
        } else {
            proof.verify_miss((report.shot_x, report.shot_y), &player.board_commitment)
        };

        if !is_valid {
            return Err("Invalid proof".to_string());
        }

        // Update player's ship state if hit
        if report.is_hit {
            if let Some(player) = self.players.get_mut(&report.player_id) {
                player.check_hit(report.shot_x, report.shot_y);
            }
        }

        Ok(is_valid)
    }

    /// Check if a player is defeated
    pub fn is_player_defeated(&self, player_id: &str) -> bool {
        self.players.get(player_id)
            .map(|p| p.is_defeated())
            .unwrap_or(false)
    }

    /// Get all active players
    pub fn get_active_players(&self) -> Vec<String> {
        self.players.iter()
            .filter(|(_, p)| !p.is_defeated())
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Advance to next round
    #[allow(dead_code)]
    pub fn next_round(&mut self) {
        self.round += 1;
    }

    /// Get game statistics
    pub fn get_stats(&self) -> GameStats {
        GameStats {
            round: self.round,
            total_players: self.players.len(),
            active_players: self.get_active_players().len(),
            total_shots: self.blockchain.get_transaction_count(),
            blockchain_length: self.blockchain.chain.len(),
        }
    }

    /// Verify the entire blockchain is valid
    pub fn verify_blockchain(&self) -> bool {
        self.blockchain.is_chain_valid()
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GameStats {
    pub round: u32,
    pub total_players: usize,
    pub active_players: usize,
    pub total_shots: usize,
    pub blockchain_length: usize,
}

impl std::fmt::Display for GameStats {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "Round: {} | Players: {}/{} | Shots: {} | Blocks: {}",
            self.round,
            self.active_players,
            self.total_players,
            self.total_shots,
            self.blockchain_length
        )
    }
}
