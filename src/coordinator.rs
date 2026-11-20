use crate::blockchain::{Blockchain, Transaction};
use crate::game::{Grid, HitReport, ShotRecord};
use crate::zk::{scalar_from_hex, scalar_to_hex, Verifier, ZkSystem};
use bls12_381::Scalar;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// What the coordinator (and every node) knows about a player. This is all
/// *public*: there are no ship positions, no salts, and no HP in here. The
/// fleet lives only in the player's own `zk::FleetSecret`.
#[derive(Debug, Clone)]
pub struct PlayerState {
    pub id: String,
    /// Commitment proven legal at registration.
    pub initial_commitment: Scalar,
    /// Commitment to the player's *current* secret state. Every accepted shot
    /// response replaces it, so old responses can never be replayed.
    pub commitment: Scalar,
    pub alive: bool,
    /// True if the player was eliminated for missing a response deadline.
    pub forfeited: bool,
    pub shots_fired: Vec<(u8, u8)>,
    /// Index into the shot log of the first shot not yet resolved for this player.
    pub next_shot: u64,
    pub hits_taken: u32,
}

impl PlayerState {
    pub fn is_defeated(&self) -> bool {
        !self.alive
    }
}

/// One verified entry of the public results ledger.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShotResponse {
    pub shot_index: u64,
    pub player_id: String,
    pub is_hit: bool,
    pub is_alive: bool,
    pub new_commitment: String,
}

/// The public result of one accepted response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShotOutcome {
    pub shot_index: u64,
    pub is_hit: bool,
    pub defeated: bool,
}

/// Coordinates the entire game including blockchain and game state
pub struct GameCoordinator {
    pub blockchain: Blockchain,
    /// Only the board *size* is meaningful here. `grid.cells` stays empty on
    /// purpose: recording who occupies which cell would leak every fleet.
    pub grid: Grid,
    pub players: HashMap<String, PlayerState>,
    pub round: u32,
    /// Every shot ever fired, in order. Each living non-shooter owes a proof
    /// for each of them.
    pub shot_log: Vec<ShotRecord>,
    /// Every verified response (the public "B: miss, C: HIT" ledger).
    pub responses: Vec<ShotResponse>,
    /// If set, a player who leaves a shot unanswered for more than this many
    /// blocks forfeits. `None` (the default) disables the deadline.
    pub response_timeout_blocks: Option<u64>,
    verifier: Option<Arc<Verifier>>,
    blockchain_path: Option<PathBuf>,
}

impl GameCoordinator {
    pub fn new(grid_size: u8, mining_difficulty: usize) -> Self {
        Self {
            blockchain: Blockchain::new(mining_difficulty),
            grid: Grid::new(grid_size),
            players: HashMap::new(),
            round: 0,
            shot_log: Vec::new(),
            responses: Vec::new(),
            response_timeout_blocks: None,
            verifier: None,
            blockchain_path: None,
        }
    }

    /// Create a new GameCoordinator with blockchain persistence
    pub fn with_persistence(
        grid_size: u8,
        mining_difficulty: usize,
        blockchain_path: PathBuf,
    ) -> Self {
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
            shot_log: Vec::new(),
            responses: Vec::new(),
            response_timeout_blocks: None,
            verifier: None,
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

    /// Use these verifying keys for all proofs. Until this is called the
    /// coordinator falls back to [`ZkSystem::insecure_dev_shared`] (INSECURE
    /// development setup; see `zk` docs).
    pub fn set_verifier(&mut self, verifier: Arc<Verifier>) {
        self.verifier = Some(verifier);
    }

    fn verifier(&self) -> Arc<Verifier> {
        self.verifier
            .clone()
            .unwrap_or_else(|| ZkSystem::insecure_dev_shared().verifier.clone())
    }

    /// Fingerprint of the verifying keys in use (see [`Verifier::fingerprint`]).
    pub fn zk_fingerprint(&self) -> String {
        self.verifier().fingerprint()
    }

    /// Register a new player from a *commitment* to their secret fleet and a
    /// zero-knowledge proof that the commitment opens to a legal fleet inside
    /// this board. The coordinator never learns where the ships are.
    pub fn register_player(
        &mut self,
        player_id: String,
        board_commitment: String,
        setup_proof: &[u8],
    ) -> Result<(), String> {
        if self.players.contains_key(&player_id) {
            return Err("Player already registered".to_string());
        }
        let commitment = scalar_from_hex(&board_commitment)
            .map_err(|e| format!("Invalid board commitment: {e}"))?;

        // A copied registration (same commitment and proof under a new name)
        // would let the copycat replay the victim's public responses.
        if self
            .players
            .values()
            .any(|p| p.initial_commitment == commitment || p.commitment == commitment)
        {
            return Err("Commitment already registered".to_string());
        }

        self.verifier()
            .verify_setup(&commitment, self.grid.size, setup_proof)?;

        // Award an initial registration shot (UTXO) to the player
        self.blockchain.award_registration_shot(&player_id);

        let state = PlayerState {
            id: player_id.clone(),
            initial_commitment: commitment,
            commitment,
            alive: true,
            forfeited: false,
            shots_fired: Vec::new(),
            // Shots fired before this player joined are not theirs to answer.
            next_shot: self.shot_log.len() as u64,
            hits_taken: 0,
        };
        self.players.insert(player_id, state);
        Ok(())
    }

    /// Allow a player to mine for shots (shots are tracked as UTXOs on-chain)
    pub fn mine_for_shots(&mut self, player_id: &str) -> Result<u32, String> {
        if !self.players.contains_key(player_id) {
            return Err("Player not found".to_string());
        }

        self.enforce_timeouts();

        // Check if player is defeated
        if self.is_player_defeated(player_id) {
            return Err("Defeated players cannot mine".to_string());
        }

        // Mine pending transactions and create shot UTXOs for the miner
        let shots_earned = self.blockchain.mine_pending_transactions(player_id);

        // Auto-save blockchain after mining
        if let Err(e) = self.save_blockchain() {
            eprintln!("Warning: Failed to save blockchain: {}", e);
        }

        Ok(shots_earned)
    }

    /// Fire a shot (spends a shot UTXO and creates a transaction)
    pub fn fire_shot(
        &mut self,
        player_id: String,
        target_x: u8,
        target_y: u8,
    ) -> Result<(), String> {
        // Ensure player exists
        if !self.players.contains_key(&player_id) {
            return Err("Player not found".to_string());
        }

        self.enforce_timeouts();

        // Check if player is defeated
        if self.is_player_defeated(&player_id) {
            return Err("Defeated players cannot fire shots".to_string());
        }

        // Spend one shot UTXO on-chain
        self.blockchain.consume_shot(&player_id)?;

        // Record shot locally for the player (for stats / UI)
        if let Some(player) = self.players.get_mut(&player_id) {
            player.shots_fired.push((target_x, target_y));
        }

        // Append to the public shot log: every other living player now owes
        // a zero-knowledge response to this shot.
        self.shot_log.push(ShotRecord {
            index: self.shot_log.len() as u64,
            shooter: player_id.clone(),
            x: target_x,
            y: target_y,
            fired_at_block: self.blockchain.chain.len() as u64,
        });

        // Create transaction representing the shot
        let transaction = Transaction::new(player_id, target_x, target_y, 0);
        self.blockchain.add_transaction(transaction);

        // Auto-save blockchain after adding transaction
        if let Err(e) = self.save_blockchain() {
            eprintln!("Warning: Failed to save blockchain: {}", e);
        }

        Ok(())
    }

    /// The next shot `player` still has to answer, if any.
    fn next_owed<'a>(&'a self, player: &PlayerState) -> Option<&'a ShotRecord> {
        self.shot_log
            .iter()
            .skip(player.next_shot as usize)
            .find(|s| s.shooter != player.id)
    }

    /// All shots `player_id` still owes a proof for, oldest first. Empty for
    /// defeated or unknown players.
    pub fn pending_shots(&self, player_id: &str) -> Vec<ShotRecord> {
        match self.players.get(player_id) {
            Some(p) if p.alive => self
                .shot_log
                .iter()
                .skip(p.next_shot as usize)
                .filter(|s| s.shooter != p.id)
                .cloned()
                .collect(),
            _ => Vec::new(),
        }
    }

    /// Verified responses for one shot.
    pub fn shot_results(&self, shot_index: u64) -> Vec<&ShotResponse> {
        self.responses
            .iter()
            .filter(|r| r.shot_index == shot_index)
            .collect()
    }

    fn is_overdue(&self, player: &PlayerState) -> bool {
        let (Some(limit), true) = (self.response_timeout_blocks, player.alive) else {
            return false;
        };
        let now = self.blockchain.chain.len() as u64;
        self.next_owed(player)
            .map(|shot| now.saturating_sub(shot.fired_at_block) > limit)
            .unwrap_or(false)
    }

    /// Eliminate every player whose oldest unanswered shot is past the
    /// deadline. Returns the ids that were just forfeited. Without this a
    /// player facing a hit could simply never respond.
    pub fn enforce_timeouts(&mut self) -> Vec<String> {
        let overdue: Vec<String> = self
            .players
            .values()
            .filter(|p| self.is_overdue(p))
            .map(|p| p.id.clone())
            .collect();
        for id in &overdue {
            if let Some(p) = self.players.get_mut(id) {
                p.alive = false;
                p.forfeited = true;
            }
        }
        overdue
    }

    /// Verify a defender's zero-knowledge response to a shot and, if valid,
    /// advance their committed state.
    ///
    /// The coordinator builds the proof's public inputs itself: the old
    /// commitment and the shot coordinates come from its own records, never
    /// from the report. Only the new commitment and the two result bits are
    /// taken from the defender, and the proof must bind all of them together.
    pub fn report_hit(&mut self, report: HitReport) -> Result<ShotOutcome, String> {
        self.enforce_timeouts();

        let player = self
            .players
            .get(&report.player_id)
            .ok_or("Player not found")?;
        if !player.alive {
            return Err("Defeated players do not respond to shots".to_string());
        }

        let shot = self
            .next_owed(player)
            .ok_or("No shot is awaiting a response from this player")?
            .clone();
        if report.shot_index != shot.index {
            return Err(format!(
                "Out-of-order response: next shot to answer is #{}, got #{}",
                shot.index, report.shot_index
            ));
        }
        if (report.shot_x, report.shot_y) != (shot.x, shot.y) {
            return Err("Report coordinates do not match the recorded shot".to_string());
        }

        let new_commitment = scalar_from_hex(&report.new_commitment)
            .map_err(|e| format!("Invalid new commitment: {e}"))?;
        if new_commitment == player.commitment {
            return Err(
                "New commitment must differ from the current one (fresh salt required)".to_string(),
            );
        }

        self.verifier().verify_shot(
            &player.commitment,
            &new_commitment,
            shot.x,
            shot.y,
            report.is_hit,
            report.is_alive,
            &report.proof,
        )?;

        let player = self
            .players
            .get_mut(&report.player_id)
            .expect("checked above");
        player.commitment = new_commitment;
        player.next_shot = shot.index + 1;
        player.alive = report.is_alive;
        if report.is_hit {
            player.hits_taken += 1;
        }
        self.responses.push(ShotResponse {
            shot_index: shot.index,
            player_id: report.player_id,
            is_hit: report.is_hit,
            is_alive: report.is_alive,
            new_commitment: scalar_to_hex(&new_commitment),
        });

        Ok(ShotOutcome {
            shot_index: shot.index,
            is_hit: report.is_hit,
            defeated: !report.is_alive,
        })
    }

    /// Check if a player is defeated (sunk, or forfeited by missing a deadline)
    pub fn is_player_defeated(&self, player_id: &str) -> bool {
        self.players
            .get(player_id)
            .map(|p| p.is_defeated() || self.is_overdue(p))
            .unwrap_or(false)
    }

    /// Get all active players
    pub fn get_active_players(&self) -> Vec<String> {
        self.players
            .iter()
            .filter(|(_, p)| !p.is_defeated() && !self.is_overdue(p))
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
