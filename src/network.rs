use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};
use crate::blockchain::{Block, Transaction, Blockchain};
use crate::coordinator::GameCoordinator;

/// Represents a peer node in the network
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Peer {
    pub address: String,
    pub port: u16,
}

impl Peer {
    pub fn new(address: String, port: u16) -> Self {
        Self { address, port }
    }

    pub fn url(&self) -> String {
        format!("http://{}:{}", self.address, self.port)
    }
}

/// Network node that manages peers and blockchain synchronization
pub struct NetworkNode {
    pub peers: Arc<RwLock<HashSet<Peer>>>,
    pub coordinator: Arc<RwLock<GameCoordinator>>,
    pub node_id: String,
    pub port: u16,
}

impl NetworkNode {
    pub fn new(node_id: String, port: u16, grid_size: u8, difficulty: usize) -> Self {
        Self {
            peers: Arc::new(RwLock::new(HashSet::new())),
            coordinator: Arc::new(RwLock::new(GameCoordinator::new(grid_size, difficulty))),
            node_id,
            port,
        }
    }

    /// Add a peer to the network
    pub async fn add_peer(&self, peer: Peer) {
        let mut peers = self.peers.write().await;
        peers.insert(peer);
    }

    /// Remove a peer from the network
    pub async fn remove_peer(&self, peer: &Peer) {
        let mut peers = self.peers.write().await;
        peers.remove(peer);
    }

    /// Get all connected peers
    pub async fn get_peers(&self) -> Vec<Peer> {
        let peers = self.peers.read().await;
        peers.iter().cloned().collect()
    }

    /// Broadcast a new block to all peers
    pub async fn broadcast_block(&self, block: &Block) -> Result<(), String> {
        let peers = self.get_peers().await;
        let client = reqwest::Client::new();

        for peer in peers {
            let url = format!("{}/api/block", peer.url());
            let _ = client
                .post(&url)
                .json(block)
                .send()
                .await;
            // Ignore errors for individual peers
        }

        Ok(())
    }

    /// Broadcast a new transaction to all peers
    pub async fn broadcast_transaction(&self, transaction: &Transaction) -> Result<(), String> {
        let peers = self.get_peers().await;
        let client = reqwest::Client::new();

        for peer in peers {
            let url = format!("{}/api/transaction", peer.url());
            let _ = client
                .post(&url)
                .json(transaction)
                .send()
                .await;
            // Ignore errors for individual peers
        }

        Ok(())
    }

    /// Synchronize blockchain with a peer
    pub async fn sync_with_peer(&self, peer: &Peer) -> Result<(), String> {
        let client = reqwest::Client::new();
        let url = format!("{}/api/blockchain", peer.url());

        let response = client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch blockchain: {}", e))?;

        let peer_blockchain: Blockchain = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse blockchain: {}", e))?;

        // Replace our chain if peer's chain is longer and valid
        let mut coordinator = self.coordinator.write().await;
        if peer_blockchain.chain.len() > coordinator.blockchain.chain.len() 
            && peer_blockchain.is_chain_valid() {
            coordinator.blockchain = peer_blockchain;
            println!("✓ Synchronized blockchain from peer {}", peer.url());
        }

        Ok(())
    }

    /// Discover and sync with all peers
    pub async fn sync_with_network(&self) -> Result<(), String> {
        let peers = self.get_peers().await;
        
        for peer in peers {
            if let Err(e) = self.sync_with_peer(&peer).await {
                eprintln!("Failed to sync with peer {}: {}", peer.url(), e);
            }
        }

        Ok(())
    }

    /// Announce this node to a peer
    pub async fn announce_to_peer(&self, peer: &Peer) -> Result<(), String> {
        let client = reqwest::Client::new();
        let url = format!("{}/api/peers", peer.url());

        let self_peer = Peer::new("localhost".to_string(), self.port);

        client
            .post(&url)
            .json(&self_peer)
            .send()
            .await
            .map_err(|e| format!("Failed to announce to peer: {}", e))?;

        Ok(())
    }
}

/// Request/Response types for API
#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterPlayerRequest {
    pub player_id: String,
    pub ships: Vec<crate::game::Ship>,
    pub board_commitment: String,
    pub salt: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FireShotRequest {
    pub player_id: String,
    pub target_x: u8,
    pub target_y: u8,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MineRequest {
    pub player_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(error: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BlockchainInfo {
    pub length: usize,
    pub difficulty: usize,
    pub pending_transactions: usize,
    pub is_valid: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NodeInfo {
    pub node_id: String,
    pub port: u16,
    pub peers_count: usize,
    pub blockchain_info: BlockchainInfo,
}
