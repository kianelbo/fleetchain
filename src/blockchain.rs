use sha2::{Sha256, Digest};
use serde::{Serialize, Deserialize};
use chrono::Utc;
use std::fmt;
use std::fs;
use std::path::Path;

/// Represents an unspent transaction output (UTXO) for a single shot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShotUtxo {
    /// Unique identifier for this UTXO
    pub id: String,
    /// Owner (player ID) who can spend this shot
    pub owner: String,
    /// Index of the block in which this UTXO was created
    pub created_in_block: u64,
    /// Whether this UTXO has been spent
    pub spent: bool,
}

/// Represents a transaction in the blockchain (a shot fired by a player)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub player_id: String,
    pub target_x: u8,
    pub target_y: u8,
    pub timestamp: i64,
    pub nonce: u64,
}

impl Transaction {
    pub fn new(player_id: String, target_x: u8, target_y: u8, nonce: u64) -> Self {
        Self {
            player_id,
            target_x,
            target_y,
            timestamp: Utc::now().timestamp(),
            nonce,
        }
    }

    #[allow(dead_code)]
    pub fn hash(&self) -> String {
        let data = serde_json::to_string(self).unwrap();
        let mut hasher = Sha256::new();
        hasher.update(data.as_bytes());
        hex::encode(hasher.finalize())
    }
}

/// Represents a block in the blockchain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub index: u64,
    pub timestamp: i64,
    pub transactions: Vec<Transaction>,
    pub previous_hash: String,
    pub hash: String,
    pub nonce: u64,
}

impl Block {
    pub fn new(index: u64, transactions: Vec<Transaction>, previous_hash: String) -> Self {
        let timestamp = Utc::now().timestamp();
        let mut block = Self {
            index,
            timestamp,
            transactions,
            previous_hash,
            hash: String::new(),
            nonce: 0,
        };
        block.hash = block.calculate_hash();
        block
    }

    pub fn calculate_hash(&self) -> String {
        let data = format!(
            "{}{}{}{}{}",
            self.index,
            self.timestamp,
            serde_json::to_string(&self.transactions).unwrap(),
            self.previous_hash,
            self.nonce
        );
        let mut hasher = Sha256::new();
        hasher.update(data.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Mine the block with proof-of-work
    pub fn mine(&mut self, difficulty: usize) {
        let target = "0".repeat(difficulty);
        while !self.hash.starts_with(&target) {
            self.nonce += 1;
            self.hash = self.calculate_hash();
        }
    }
}

impl fmt::Display for Block {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Block #{} [{}...] with {} transactions",
            self.index,
            &self.hash[..8],
            self.transactions.len()
        )
    }
}

/// The blockchain itself
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Blockchain {
    pub chain: Vec<Block>,
    pub difficulty: usize,
    pub pending_transactions: Vec<Transaction>,
    pub mining_reward: u32,
    /// UTXO set representing unspent shot rewards
    pub shot_utxos: Vec<ShotUtxo>,
}

impl Blockchain {
    pub fn new(difficulty: usize) -> Self {
        let mut blockchain = Self {
            chain: Vec::new(),
            difficulty,
            pending_transactions: Vec::new(),
            mining_reward: 1,
            shot_utxos: Vec::new(),
        };
        blockchain.create_genesis_block();
        blockchain
    }

    fn create_genesis_block(&mut self) {
        let genesis = Block::new(0, Vec::new(), String::from("0"));
        self.chain.push(genesis);
    }

    pub fn get_latest_block(&self) -> &Block {
        self.chain.last().unwrap()
    }

    pub fn add_transaction(&mut self, transaction: Transaction) {
        self.pending_transactions.push(transaction);
    }

    /// Mine pending transactions and reward the miner with shot UTXOs
    pub fn mine_pending_transactions(&mut self, miner_address: &str) -> u32 {
        let next_index = self.chain.len() as u64;
        let mut block = Block::new(
            next_index,
            self.pending_transactions.clone(),
            self.get_latest_block().hash.clone(),
        );

        block.mine(self.difficulty);
        let block_hash = block.hash.clone();
        self.chain.push(block);
        self.pending_transactions.clear();

        // Create shot UTXOs for the miner
        for i in 0..self.mining_reward {
            let utxo_id_input = format!("{}:{}:{}", miner_address, block_hash, i);
            let mut hasher = Sha256::new();
            hasher.update(utxo_id_input.as_bytes());
            let id = hex::encode(hasher.finalize());

            self.shot_utxos.push(ShotUtxo {
                id,
                owner: miner_address.to_string(),
                created_in_block: next_index,
                spent: false,
            });
        }

        // Return the number of shots earned
        self.mining_reward
    }

    pub fn is_chain_valid(&self) -> bool {
        for i in 1..self.chain.len() {
            let current_block = &self.chain[i];
            let previous_block = &self.chain[i - 1];

            if current_block.hash != current_block.calculate_hash() {
                return false;
            }

            if current_block.previous_hash != previous_block.hash {
                return false;
            }

            if !current_block.hash.starts_with(&"0".repeat(self.difficulty)) {
                return false;
            }
        }
        true
    }

    pub fn get_transaction_count(&self) -> usize {
        self.chain.iter().map(|block| block.transactions.len()).sum()
    }

    /// Get the number of unspent shot UTXOs for a given player
    pub fn get_unspent_shots(&self, player_id: &str) -> usize {
        self.shot_utxos
            .iter()
            .filter(|u| u.owner == player_id && !u.spent)
            .count()
    }

    /// Consume a single shot UTXO for the given player
    pub fn consume_shot(&mut self, player_id: &str) -> Result<(), String> {
        if let Some(utxo) = self
            .shot_utxos
            .iter_mut()
            .find(|u| u.owner == player_id && !u.spent)
        {
            utxo.spent = true;
            Ok(())
        } else {
            Err("No unspent shot UTXOs available".to_string())
        }
    }

    /// Award a single registration shot UTXO to a player
    pub fn award_registration_shot(&mut self, player_id: &str) {
        let latest_block = self.get_latest_block();
        let utxo_id_input = format!("{}:{}:registration", player_id, latest_block.hash);
        let mut hasher = Sha256::new();
        hasher.update(utxo_id_input.as_bytes());
        let id = hex::encode(hasher.finalize());

        self.shot_utxos.push(ShotUtxo {
            id,
            owner: player_id.to_string(),
            created_in_block: latest_block.index,
            spent: false,
        });
    }

    /// Save the blockchain to a JSON file
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize blockchain: {}", e))?;

        fs::write(path, json)
            .map_err(|e| format!("Failed to write blockchain file: {}", e))?;

        Ok(())
    }

    /// Load the blockchain from a JSON file
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let json = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read blockchain file: {}", e))?;

        let blockchain: Blockchain = serde_json::from_str(&json)
            .map_err(|e| format!("Failed to deserialize blockchain: {}", e))?;

        // Verify the loaded blockchain is valid
        if !blockchain.is_chain_valid() {
            return Err("Loaded blockchain is invalid".to_string());
        }

        Ok(blockchain)
    }

    /// Check if a blockchain file exists
    pub fn file_exists<P: AsRef<Path>>(path: P) -> bool {
        path.as_ref().exists()
    }
}
