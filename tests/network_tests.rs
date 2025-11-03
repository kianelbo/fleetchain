use fleetchain::network::{NetworkNode, Peer};
use fleetchain::game::Ship;
use fleetchain::crypto::{generate_salt, create_commitment};
use fleetchain::blockchain::Transaction;

#[tokio::test]
async fn test_network_node_creation() {
    let node = NetworkNode::new("node1".to_string(), 8080, 10, 2);
    
    assert_eq!(node.node_id, "node1");
    assert_eq!(node.port, 8080);
    
    let coordinator = node.coordinator.read().await;
    assert_eq!(coordinator.grid.size, 10);
    assert_eq!(coordinator.blockchain.difficulty, 2);
}

#[tokio::test]
async fn test_add_peer() {
    let node = NetworkNode::new("node1".to_string(), 8080, 10, 2);
    let peer = Peer::new("localhost".to_string(), 8081);
    
    node.add_peer(peer.clone()).await;
    
    let peers = node.get_peers().await;
    assert_eq!(peers.len(), 1);
    assert!(peers.contains(&peer));
}

#[tokio::test]
async fn test_add_multiple_peers() {
    let node = NetworkNode::new("node1".to_string(), 8080, 10, 2);
    
    for i in 1..5 {
        let peer = Peer::new("localhost".to_string(), 8080 + i);
        node.add_peer(peer).await;
    }
    
    let peers = node.get_peers().await;
    assert_eq!(peers.len(), 4);
}

#[tokio::test]
async fn test_remove_peer() {
    let node = NetworkNode::new("node1".to_string(), 8080, 10, 2);
    let peer = Peer::new("localhost".to_string(), 8081);
    
    node.add_peer(peer.clone()).await;
    assert_eq!(node.get_peers().await.len(), 1);
    
    node.remove_peer(&peer).await;
    assert_eq!(node.get_peers().await.len(), 0);
}

#[tokio::test]
async fn test_peer_uniqueness() {
    let node = NetworkNode::new("node1".to_string(), 8080, 10, 2);
    let peer = Peer::new("localhost".to_string(), 8081);
    
    // Add same peer twice
    node.add_peer(peer.clone()).await;
    node.add_peer(peer.clone()).await;
    
    // Should only have one peer (HashSet ensures uniqueness)
    let peers = node.get_peers().await;
    assert_eq!(peers.len(), 1);
}

#[tokio::test]
async fn test_peer_url_generation() {
    let peer = Peer::new("localhost".to_string(), 8080);
    assert_eq!(peer.url(), "http://localhost:8080");
    
    let peer2 = Peer::new("192.168.1.1".to_string(), 3000);
    assert_eq!(peer2.url(), "http://192.168.1.1:3000");
}

#[tokio::test]
async fn test_peer_equality() {
    let peer1 = Peer::new("localhost".to_string(), 8080);
    let peer2 = Peer::new("localhost".to_string(), 8080);
    let peer3 = Peer::new("localhost".to_string(), 8081);
    
    assert_eq!(peer1, peer2);
    assert_ne!(peer1, peer3);
}

#[tokio::test]
async fn test_peer_serialization() {
    let peer = Peer::new("localhost".to_string(), 8080);
    let json = serde_json::to_string(&peer).unwrap();
    let deserialized: Peer = serde_json::from_str(&json).unwrap();
    
    assert_eq!(peer, deserialized);
}

#[tokio::test]
async fn test_concurrent_peer_operations() {
    let node = std::sync::Arc::new(NetworkNode::new("node1".to_string(), 8080, 10, 2));
    
    let mut handles = vec![];
    
    // Spawn multiple tasks adding peers concurrently
    for i in 0..10 {
        let node_clone = node.clone();
        let handle = tokio::spawn(async move {
            let peer = Peer::new("localhost".to_string(), 8080 + i);
            node_clone.add_peer(peer).await;
        });
        handles.push(handle);
    }
    
    // Wait for all tasks
    for handle in handles {
        handle.await.unwrap();
    }
    
    let peers = node.get_peers().await;
    assert_eq!(peers.len(), 10);
}

#[tokio::test]
async fn test_node_with_game_state() {
    let node = NetworkNode::new("node1".to_string(), 8080, 10, 2);
    
    let ships = vec![Ship::new("carrier".to_string(), vec![(0, 0), (0, 1)])];
    let positions: Vec<(u8, u8)> = ships.iter()
        .flat_map(|s| s.positions.clone())
        .collect();
    let salt = generate_salt();
    let commitment = create_commitment(&positions, &salt);
    
    let mut coordinator = node.coordinator.write().await;
    let result = coordinator.register_player(
        "player1".to_string(),
        ships,
        commitment,
        salt,
    );
    
    assert!(result.is_ok());
    assert_eq!(coordinator.players.len(), 1);
}

#[tokio::test]
async fn test_node_mining() {
    let node = NetworkNode::new("node1".to_string(), 8080, 10, 2);
    
    let ships = vec![Ship::new("carrier".to_string(), vec![(0, 0)])];
    let positions: Vec<(u8, u8)> = ships.iter()
        .flat_map(|s| s.positions.clone())
        .collect();
    let salt = generate_salt();
    let commitment = create_commitment(&positions, &salt);
    
    let mut coordinator = node.coordinator.write().await;
    coordinator.register_player("player1".to_string(), ships, commitment, salt).unwrap();
    
    let initial_length = coordinator.blockchain.chain.len();
    coordinator.mine_for_shots("player1").unwrap();
    
    assert_eq!(coordinator.blockchain.chain.len(), initial_length + 1);
}

#[tokio::test]
async fn test_node_transaction_handling() {
    let node = NetworkNode::new("node1".to_string(), 8080, 10, 2);
    
    let ships = vec![Ship::new("carrier".to_string(), vec![(0, 0)])];
    let positions: Vec<(u8, u8)> = ships.iter()
        .flat_map(|s| s.positions.clone())
        .collect();
    let salt = generate_salt();
    let commitment = create_commitment(&positions, &salt);
    
    let mut coordinator = node.coordinator.write().await;
    coordinator.register_player("player1".to_string(), ships, commitment, salt).unwrap();
    coordinator.mine_for_shots("player1").unwrap();
    
    coordinator.fire_shot("player1".to_string(), 5, 5).unwrap();
    
    assert_eq!(coordinator.blockchain.pending_transactions.len(), 1);
}

#[tokio::test]
async fn test_multiple_nodes_independent_state() {
    let node1 = NetworkNode::new("node1".to_string(), 8080, 10, 2);
    let node2 = NetworkNode::new("node2".to_string(), 8081, 10, 2);
    
    // Register player on node1
    let ships = vec![Ship::new("carrier".to_string(), vec![(0, 0)])];
    let positions: Vec<(u8, u8)> = ships.iter()
        .flat_map(|s| s.positions.clone())
        .collect();
    let salt = generate_salt();
    let commitment = create_commitment(&positions, &salt);
    
    let mut coordinator1 = node1.coordinator.write().await;
    coordinator1.register_player("player1".to_string(), ships, commitment, salt).unwrap();
    drop(coordinator1);
    
    // Node2 should have no players
    let coordinator2 = node2.coordinator.read().await;
    assert_eq!(coordinator2.players.len(), 0);
}

#[tokio::test]
async fn test_peer_list_operations() {
    let node = NetworkNode::new("node1".to_string(), 8080, 10, 2);
    
    // Add peers
    for i in 1..4 {
        node.add_peer(Peer::new("localhost".to_string(), 8080 + i)).await;
    }
    
    let peers = node.get_peers().await;
    assert_eq!(peers.len(), 3);
    
    // Remove one peer
    let peer_to_remove = Peer::new("localhost".to_string(), 8081);
    node.remove_peer(&peer_to_remove).await;
    
    let peers = node.get_peers().await;
    assert_eq!(peers.len(), 2);
    assert!(!peers.contains(&peer_to_remove));
}

#[tokio::test]
async fn test_blockchain_state_isolation() {
    let node1 = NetworkNode::new("node1".to_string(), 8080, 10, 2);
    let node2 = NetworkNode::new("node2".to_string(), 8081, 10, 3);
    
    let coordinator1 = node1.coordinator.read().await;
    let coordinator2 = node2.coordinator.read().await;
    
    assert_eq!(coordinator1.blockchain.difficulty, 2);
    assert_eq!(coordinator2.blockchain.difficulty, 3);
}

#[tokio::test]
async fn test_node_stats_access() {
    let node = NetworkNode::new("node1".to_string(), 8080, 10, 2);
    
    let coordinator = node.coordinator.read().await;
    let stats = coordinator.get_stats();
    
    assert_eq!(stats.total_players, 0);
    assert_eq!(stats.round, 0);
    assert!(stats.blockchain_length > 0); // Genesis block
}

#[tokio::test]
async fn test_concurrent_blockchain_access() {
    let node = std::sync::Arc::new(NetworkNode::new("node1".to_string(), 8080, 10, 2));
    
    let mut handles = vec![];
    
    // Multiple readers
    for _ in 0..5 {
        let node_clone = node.clone();
        let handle = tokio::spawn(async move {
            let coordinator = node_clone.coordinator.read().await;
            coordinator.blockchain.is_chain_valid()
        });
        handles.push(handle);
    }
    
    for handle in handles {
        let is_valid = handle.await.unwrap();
        assert!(is_valid);
    }
}

#[tokio::test]
async fn test_node_id_uniqueness() {
    let node1 = NetworkNode::new("node1".to_string(), 8080, 10, 2);
    let node2 = NetworkNode::new("node2".to_string(), 8081, 10, 2);
    let node3 = NetworkNode::new("node1".to_string(), 8082, 10, 2); // Same ID as node1
    
    assert_ne!(node1.node_id, node2.node_id);
    assert_eq!(node1.node_id, node3.node_id);
}

#[tokio::test]
async fn test_large_peer_network() {
    let node = NetworkNode::new("node1".to_string(), 8080, 10, 2);
    
    // Add 100 peers
    for i in 1..101 {
        node.add_peer(Peer::new("localhost".to_string(), 8000 + i)).await;
    }
    
    let peers = node.get_peers().await;
    assert_eq!(peers.len(), 100);
}

#[tokio::test]
async fn test_peer_different_addresses() {
    let node = NetworkNode::new("node1".to_string(), 8080, 10, 2);
    
    node.add_peer(Peer::new("localhost".to_string(), 8081)).await;
    node.add_peer(Peer::new("127.0.0.1".to_string(), 8081)).await;
    node.add_peer(Peer::new("192.168.1.1".to_string(), 8081)).await;
    
    let peers = node.get_peers().await;
    assert_eq!(peers.len(), 3);
}

#[tokio::test]
async fn test_transaction_creation_and_storage() {
    let tx = Transaction::new("player1".to_string(), 5, 5, 0);
    
    assert_eq!(tx.player_id, "player1");
    assert_eq!(tx.target_x, 5);
    assert_eq!(tx.target_y, 5);
    assert_eq!(tx.nonce, 0);
}

#[tokio::test]
async fn test_node_blockchain_validity() {
    let node = NetworkNode::new("node1".to_string(), 8080, 10, 2);
    
    let coordinator = node.coordinator.read().await;
    assert!(coordinator.verify_blockchain());
}

#[tokio::test]
async fn test_empty_peer_list() {
    let node = NetworkNode::new("node1".to_string(), 8080, 10, 2);
    
    let peers = node.get_peers().await;
    assert_eq!(peers.len(), 0);
}

#[tokio::test]
async fn test_node_configuration() {
    let node = NetworkNode::new("test_node".to_string(), 9999, 20, 5);
    
    assert_eq!(node.node_id, "test_node");
    assert_eq!(node.port, 9999);
    
    let coordinator = node.coordinator.read().await;
    assert_eq!(coordinator.grid.size, 20);
    assert_eq!(coordinator.blockchain.difficulty, 5);
}
