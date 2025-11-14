use fleetchain::blockchain::{Blockchain, Transaction, Block};
use std::fs;
use std::path::PathBuf;

#[test]
fn test_genesis_block_creation() {
    let blockchain = Blockchain::new(2);
    assert_eq!(blockchain.chain.len(), 1);
    assert_eq!(blockchain.chain[0].index, 0);
    assert_eq!(blockchain.chain[0].transactions.len(), 0);
    assert_eq!(blockchain.chain[0].previous_hash, "0");
}

#[test]
fn test_blockchain_validation() {
    let blockchain = Blockchain::new(2);
    assert!(blockchain.is_chain_valid());
}

#[test]
fn test_add_transaction() {
    let mut blockchain = Blockchain::new(2);
    let tx = Transaction::new("player1".to_string(), 5, 5, 0);
    
    blockchain.add_transaction(tx);
    assert_eq!(blockchain.pending_transactions.len(), 1);
}

#[test]
fn test_mining_creates_new_block() {
    let mut blockchain = Blockchain::new(2);
    let tx = Transaction::new("player1".to_string(), 5, 5, 0);
    blockchain.add_transaction(tx);
    
    let initial_length = blockchain.chain.len();
    blockchain.mine_pending_transactions("miner1");
    
    assert_eq!(blockchain.chain.len(), initial_length + 1);
    assert_eq!(blockchain.pending_transactions.len(), 0);
}

#[test]
fn test_mined_block_has_correct_proof_of_work() {
    let mut blockchain = Blockchain::new(3);
    let tx = Transaction::new("player1".to_string(), 5, 5, 0);
    blockchain.add_transaction(tx);
    
    blockchain.mine_pending_transactions("miner1");
    let latest_block = blockchain.get_latest_block();
    
    assert!(latest_block.hash.starts_with("000"));
}

#[test]
fn test_block_hash_changes_with_nonce() {
    let mut block1 = Block::new(1, vec![], "prev_hash".to_string());
    let hash1 = block1.hash.clone();
    
    block1.nonce = 12345;
    let hash2 = block1.calculate_hash();
    
    assert_ne!(hash1, hash2);
}

#[test]
fn test_blockchain_rejects_invalid_previous_hash() {
    let mut blockchain = Blockchain::new(2);
    
    // Create a block with wrong previous hash
    let mut bad_block = Block::new(
        blockchain.chain.len() as u64,
        vec![],
        "wrong_hash".to_string(),
    );
    bad_block.mine(2);
    blockchain.chain.push(bad_block);
    
    assert!(!blockchain.is_chain_valid());
}

#[test]
fn test_blockchain_rejects_tampered_block() {
    let mut blockchain = Blockchain::new(2);
    let tx = Transaction::new("player1".to_string(), 5, 5, 0);
    blockchain.add_transaction(tx);
    blockchain.mine_pending_transactions("miner1");
    
    // Tamper with a block
    blockchain.chain[1].transactions.push(
        Transaction::new("hacker".to_string(), 9, 9, 0)
    );
    
    assert!(!blockchain.is_chain_valid());
}

#[test]
fn test_blockchain_with_multiple_blocks() {
    let mut blockchain = Blockchain::new(2);
    
    for i in 0..5 {
        let tx = Transaction::new(format!("player{}", i), i as u8, i as u8, 0);
        blockchain.add_transaction(tx);
        blockchain.mine_pending_transactions(&format!("miner{}", i));
    }
    
    assert_eq!(blockchain.chain.len(), 6); // Genesis + 5 blocks
    assert!(blockchain.is_chain_valid());
}

#[test]
fn test_transaction_serialization() {
    let tx = Transaction::new("player1".to_string(), 5, 5, 42);
    let json = serde_json::to_string(&tx).unwrap();
    let deserialized: Transaction = serde_json::from_str(&json).unwrap();
    
    assert_eq!(tx.player_id, deserialized.player_id);
    assert_eq!(tx.target_x, deserialized.target_x);
    assert_eq!(tx.target_y, deserialized.target_y);
    assert_eq!(tx.nonce, deserialized.nonce);
}

#[test]
fn test_block_serialization() {
    let tx = Transaction::new("player1".to_string(), 5, 5, 0);
    let block = Block::new(1, vec![tx], "prev_hash".to_string());
    
    let json = serde_json::to_string(&block).unwrap();
    let deserialized: Block = serde_json::from_str(&json).unwrap();
    
    assert_eq!(block.index, deserialized.index);
    assert_eq!(block.transactions.len(), deserialized.transactions.len());
    assert_eq!(block.previous_hash, deserialized.previous_hash);
}

#[test]
fn test_blockchain_serialization() {
    let mut blockchain = Blockchain::new(2);
    let tx = Transaction::new("player1".to_string(), 5, 5, 0);
    blockchain.add_transaction(tx);
    blockchain.mine_pending_transactions("miner1");
    
    let json = serde_json::to_string(&blockchain).unwrap();
    let deserialized: Blockchain = serde_json::from_str(&json).unwrap();
    
    assert_eq!(blockchain.chain.len(), deserialized.chain.len());
    assert_eq!(blockchain.difficulty, deserialized.difficulty);
}

#[test]
fn test_mining_difficulty_affects_hash() {
    let mut blockchain_easy = Blockchain::new(1);
    let mut blockchain_hard = Blockchain::new(4);
    
    let tx1 = Transaction::new("player1".to_string(), 5, 5, 0);
    let tx2 = Transaction::new("player1".to_string(), 5, 5, 0);
    
    blockchain_easy.add_transaction(tx1);
    blockchain_hard.add_transaction(tx2);
    
    blockchain_easy.mine_pending_transactions("miner1");
    blockchain_hard.mine_pending_transactions("miner1");
    
    let easy_hash = &blockchain_easy.get_latest_block().hash;
    let hard_hash = &blockchain_hard.get_latest_block().hash;
    
    assert!(easy_hash.starts_with("0"));
    assert!(hard_hash.starts_with("0000"));
}

#[test]
fn test_get_transaction_count() {
    let mut blockchain = Blockchain::new(2);
    
    for i in 0..3 {
        let tx = Transaction::new(format!("player{}", i), i as u8, i as u8, 0);
        blockchain.add_transaction(tx);
        blockchain.mine_pending_transactions(&format!("miner{}", i));
    }
    
    assert_eq!(blockchain.get_transaction_count(), 3);
}

#[test]
fn test_empty_block_mining() {
    let mut blockchain = Blockchain::new(2);
    blockchain.mine_pending_transactions("miner1");
    
    assert_eq!(blockchain.chain.len(), 2);
    assert_eq!(blockchain.chain[1].transactions.len(), 0);
}

#[test]
fn test_multiple_transactions_per_block() {
    let mut blockchain = Blockchain::new(2);
    
    for i in 0..5 {
        let tx = Transaction::new(format!("player{}", i), i as u8, i as u8, 0);
        blockchain.add_transaction(tx);
    }
    
    blockchain.mine_pending_transactions("miner1");
    
    assert_eq!(blockchain.chain[1].transactions.len(), 5);
    assert_eq!(blockchain.pending_transactions.len(), 0);
}

#[test]
fn test_blockchain_immutability() {
    let mut blockchain = Blockchain::new(2);
    let tx = Transaction::new("player1".to_string(), 5, 5, 0);
    blockchain.add_transaction(tx);
    blockchain.mine_pending_transactions("miner1");
    
    let original_hash = blockchain.chain[1].hash.clone();
    
    // Try to tamper
    blockchain.chain[1].nonce += 1;
    
    // Hash should be different now, making chain invalid
    assert_ne!(blockchain.chain[1].calculate_hash(), original_hash);
    assert!(!blockchain.is_chain_valid());
}

#[test]
fn test_blockchain_persistence_save_and_load() {
    let test_path = PathBuf::from("test_blockchain_save.json");
    
    // Clean up any existing test file
    let _ = fs::remove_file(&test_path);
    
    // Create and save blockchain
    let mut blockchain = Blockchain::new(2);
    let tx1 = Transaction::new("player1".to_string(), 5, 5, 0);
    let tx2 = Transaction::new("player2".to_string(), 3, 7, 0);
    blockchain.add_transaction(tx1);
    blockchain.add_transaction(tx2);
    blockchain.mine_pending_transactions("miner1");
    
    let original_length = blockchain.chain.len();
    let original_tx_count = blockchain.get_transaction_count();
    
    // Save to file
    blockchain.save_to_file(&test_path).expect("Failed to save blockchain");
    assert!(test_path.exists());
    
    // Load from file
    let loaded_blockchain = Blockchain::load_from_file(&test_path)
        .expect("Failed to load blockchain");
    
    // Verify loaded blockchain matches original
    assert_eq!(loaded_blockchain.chain.len(), original_length);
    assert_eq!(loaded_blockchain.get_transaction_count(), original_tx_count);
    assert_eq!(loaded_blockchain.difficulty, blockchain.difficulty);
    assert!(loaded_blockchain.is_chain_valid());
    
    // Clean up
    fs::remove_file(&test_path).ok();
}

#[test]
fn test_blockchain_persistence_file_exists() {
    let test_path = PathBuf::from("test_blockchain_exists.json");
    
    // Clean up any existing test file
    let _ = fs::remove_file(&test_path);
    
    assert!(!Blockchain::file_exists(&test_path));
    
    let blockchain = Blockchain::new(2);
    blockchain.save_to_file(&test_path).expect("Failed to save blockchain");
    
    assert!(Blockchain::file_exists(&test_path));
    
    // Clean up
    fs::remove_file(&test_path).ok();
}

#[test]
fn test_blockchain_persistence_validates_on_load() {
    let test_path = PathBuf::from("test_blockchain_invalid.json");
    
    // Clean up any existing test file
    let _ = fs::remove_file(&test_path);
    
    // Create a blockchain and save it
    let mut blockchain = Blockchain::new(2);
    let tx = Transaction::new("player1".to_string(), 5, 5, 0);
    blockchain.add_transaction(tx);
    blockchain.mine_pending_transactions("miner1");
    blockchain.save_to_file(&test_path).expect("Failed to save blockchain");
    
    // Load it successfully first
    let loaded = Blockchain::load_from_file(&test_path);
    assert!(loaded.is_ok(), "Should load valid blockchain");
    
    // Manually tamper with the file - change the nonce to invalidate proof of work
    let json = fs::read_to_string(&test_path).unwrap();
    let mut blockchain_data: serde_json::Value = serde_json::from_str(&json).unwrap();
    
    // Tamper with the first non-genesis block's nonce
    if let Some(block) = blockchain_data["chain"].as_array_mut().and_then(|arr| arr.get_mut(1)) {
        block["nonce"] = serde_json::json!(999999);
    }
    
    fs::write(&test_path, serde_json::to_string_pretty(&blockchain_data).unwrap()).unwrap();
    
    // Try to load the tampered blockchain - should fail validation
    let result = Blockchain::load_from_file(&test_path);
    assert!(result.is_err(), "Expected tampered blockchain to fail validation");
    
    // Clean up
    fs::remove_file(&test_path).ok();
}

#[test]
fn test_blockchain_persistence_with_multiple_blocks() {
    let test_path = PathBuf::from("test_blockchain_multi.json");
    
    // Clean up any existing test file
    let _ = fs::remove_file(&test_path);
    
    // Create blockchain with multiple blocks
    let mut blockchain = Blockchain::new(2);
    for i in 0..5 {
        let tx = Transaction::new(format!("player{}", i), i as u8, i as u8, 0);
        blockchain.add_transaction(tx);
        blockchain.mine_pending_transactions(&format!("miner{}", i));
    }
    
    let original_length = blockchain.chain.len();
    
    // Save and reload
    blockchain.save_to_file(&test_path).expect("Failed to save blockchain");
    let loaded = Blockchain::load_from_file(&test_path).expect("Failed to load blockchain");
    
    assert_eq!(loaded.chain.len(), original_length);
    assert!(loaded.is_chain_valid());
    
    // Verify each block
    for i in 0..original_length {
        assert_eq!(loaded.chain[i].index, blockchain.chain[i].index);
        assert_eq!(loaded.chain[i].hash, blockchain.chain[i].hash);
        assert_eq!(loaded.chain[i].previous_hash, blockchain.chain[i].previous_hash);
    }
    
    // Clean up
    fs::remove_file(&test_path).ok();
}
