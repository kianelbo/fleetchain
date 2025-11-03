use crate::blockchain::{Blockchain, Transaction};
use crate::game::{Grid, Player, Ship, HitReport};
use crate::crypto::{create_commitment, verify_commitment, HitProof};
use std::collections::HashMap;

/// Coordinates the entire game including blockchain and game state
pub struct GameCoordinator {
    pub blockchain: Blockchain,
    pub grid: Grid,
    pub players: HashMap<String, Player>,
    pub round: u32,
}

impl GameCoordinator {
    pub fn new(grid_size: u8, mining_difficulty: usize) -> Self {
        Self {
            blockchain: Blockchain::new(mining_difficulty),
            grid: Grid::new(grid_size),
            players: HashMap::new(),
            round: 0,
        }
    }

    /// Register a new player with their fleet
    pub fn register_player(
        &mut self,
        player_id: String,
        ships: Vec<Ship>,
        board_commitment: String,
        salt: String,
    ) -> Result<(), String> {
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

    /// Allow a player to mine for shots
    pub fn mine_for_shots(&mut self, player_id: &str) -> Result<u32, String> {
        if !self.players.contains_key(player_id) {
            return Err("Player not found".to_string());
        }

        // Mine pending transactions
        let shots_earned = self.blockchain.mine_pending_transactions(player_id);

        // Award shots to the miner
        if let Some(player) = self.players.get_mut(player_id) {
            player.add_shots(shots_earned);
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

        Ok(())
    }

    /// Process hit reports with ZK proofs
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::generate_salt;

    #[test]
    fn test_player_registration() {
        let mut coordinator = GameCoordinator::new(10, 2);
        
        let ships = vec![
            Ship::new("ship1".to_string(), vec![(0, 0), (0, 1), (0, 2)]),
        ];

        let all_positions: Vec<(u8, u8)> = ships.iter()
            .flat_map(|ship| ship.positions.clone())
            .collect();

        let salt = generate_salt();
        let commitment = create_commitment(&all_positions, &salt);

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
    fn test_mining_and_shooting() {
        let mut coordinator = GameCoordinator::new(10, 2);

        let ships = vec![
            Ship::new("ship1".to_string(), vec![(0, 0), (0, 1)]),
        ];

        let all_positions: Vec<(u8, u8)> = ships.iter()
            .flat_map(|ship| ship.positions.clone())
            .collect();

        let salt = generate_salt();
        let commitment = create_commitment(&all_positions, &salt);

        coordinator.register_player(
            "player1".to_string(),
            ships,
            commitment,
            salt,
        ).unwrap();

        // Mine for shots
        let shots = coordinator.mine_for_shots("player1").unwrap();
        assert!(shots > 0);

        // Fire a shot
        let result = coordinator.fire_shot("player1".to_string(), 5, 5);
        assert!(result.is_ok());
    }
}
