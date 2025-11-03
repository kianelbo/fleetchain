use sha2::{Sha256, Digest};
use serde::{Serialize, Deserialize};
use chrono::Utc;
use std::fmt;

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
}

impl Blockchain {
    pub fn new(difficulty: usize) -> Self {
        let mut blockchain = Self {
            chain: Vec::new(),
            difficulty,
            pending_transactions: Vec::new(),
            mining_reward: 1,
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

    /// Mine pending transactions and reward the miner with shots
    pub fn mine_pending_transactions(&mut self, miner_address: &str) -> u32 {
        let mut block = Block::new(
            self.chain.len() as u64,
            self.pending_transactions.clone(),
            self.get_latest_block().hash.clone(),
        );

        block.mine(self.difficulty);
        self.chain.push(block);
        self.pending_transactions.clear();

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
}
