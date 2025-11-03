use serde::{Serialize, Deserialize};
use std::collections::HashMap;

/// Represents a ship on the grid
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Ship {
    pub id: String,
    pub positions: Vec<(u8, u8)>,
    pub hits: Vec<bool>,
}

impl Ship {
    pub fn new(id: String, positions: Vec<(u8, u8)>) -> Self {
        let hits = vec![false; positions.len()];
        Self { id, positions, hits }
    }

    pub fn is_hit_at(&self, x: u8, y: u8) -> bool {
        self.positions.iter().position(|&pos| pos == (x, y))
            .map(|idx| self.hits[idx])
            .unwrap_or(false)
    }

    pub fn register_hit(&mut self, x: u8, y: u8) -> bool {
        if let Some(idx) = self.positions.iter().position(|&pos| pos == (x, y)) {
            self.hits[idx] = true;
            true
        } else {
            false
        }
    }

    pub fn is_sunk(&self) -> bool {
        self.hits.iter().all(|&hit| hit)
    }
}

/// Represents the game grid
#[derive(Debug, Clone)]
pub struct Grid {
    pub size: u8,
    // Maps cell coordinates to list of player IDs who have ships there
    pub cells: HashMap<(u8, u8), Vec<String>>,
}

impl Grid {
    pub fn new(size: u8) -> Self {
        Self {
            size,
            cells: HashMap::new(),
        }
    }

    pub fn place_ship(&mut self, player_id: &str, positions: &[(u8, u8)]) -> Result<(), String> {
        for &(x, y) in positions {
            if x >= self.size || y >= self.size {
                return Err(format!("Position ({}, {}) is out of bounds", x, y));
            }
        }

        for &pos in positions {
            self.cells.entry(pos)
                .or_insert_with(Vec::new)
                .push(player_id.to_string());
        }

        Ok(())
    }

    pub fn get_players_at(&self, x: u8, y: u8) -> Vec<String> {
        self.cells.get(&(x, y))
            .cloned()
            .unwrap_or_default()
    }
}

/// Represents a player in the game
#[derive(Debug, Clone)]
pub struct Player {
    pub id: String,
    pub ships: Vec<Ship>,
    pub board_commitment: String,
    pub salt: String,
    pub shots_available: u32,
    pub shots_fired: Vec<(u8, u8)>,
}

impl Player {
    pub fn new(id: String, ships: Vec<Ship>, board_commitment: String, salt: String) -> Self {
        Self {
            id,
            ships,
            board_commitment,
            salt,
            shots_available: 0,
            shots_fired: Vec::new(),
        }
    }

    pub fn add_shots(&mut self, count: u32) {
        self.shots_available += count;
    }

    pub fn fire_shot(&mut self, x: u8, y: u8) -> Result<(), String> {
        if self.shots_available == 0 {
            return Err("No shots available".to_string());
        }
        self.shots_available -= 1;
        self.shots_fired.push((x, y));
        Ok(())
    }

    pub fn check_hit(&mut self, x: u8, y: u8) -> bool {
        for ship in &mut self.ships {
            if ship.register_hit(x, y) {
                return true;
            }
        }
        false
    }

    pub fn is_defeated(&self) -> bool {
        self.ships.iter().all(|ship| ship.is_sunk())
    }

    pub fn get_all_ship_positions(&self) -> Vec<(u8, u8)> {
        self.ships.iter()
            .flat_map(|ship| ship.positions.clone())
            .collect()
    }
}

/// Hit report with proof
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HitReport {
    pub player_id: String,
    pub shot_x: u8,
    pub shot_y: u8,
    pub is_hit: bool,
    pub proof: Vec<u8>, // ZK proof data
}

impl HitReport {
    pub fn new(player_id: String, shot_x: u8, shot_y: u8, is_hit: bool, proof: Vec<u8>) -> Self {
        Self {
            player_id,
            shot_x,
            shot_y,
            is_hit,
            proof,
        }
    }
}
