use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::util::ServiceExt;
use std::sync::Arc;
use fleetchain::api::create_router;
use fleetchain::network::{NetworkNode, RegisterPlayerRequest, FireShotRequest, MineRequest, Peer};
use fleetchain::game::Ship;
use fleetchain::crypto::{generate_salt, create_commitment};
use fleetchain::blockchain::{Block, Transaction};

// Helper function to create a valid 4-ship fleet
fn create_valid_fleet() -> Vec<Ship> {
    vec![
        Ship::new("Carrier".to_string(), vec![(0, 0), (0, 1), (0, 2), (0, 3)]),
        Ship::new("Cruiser".to_string(), vec![(2, 0), (2, 1), (2, 2)]),
        Ship::new("Submarine".to_string(), vec![(4, 0), (4, 1)]),
        Ship::new("Destroyer".to_string(), vec![(6, 0)]),
    ]
}

#[tokio::test]
async fn test_get_blockchain() {
    let node = Arc::new(NetworkNode::new("test_node".to_string(), 8080, 10, 2));
    let app = create_router(node.clone());

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/blockchain")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_register_player() {
    let node = Arc::new(NetworkNode::new("test_node".to_string(), 8080, 10, 2));
    let app = create_router(node.clone());

    let ships = create_valid_fleet();

    let all_positions: Vec<(u8, u8)> = ships.iter()
        .flat_map(|ship| ship.positions.clone())
        .collect();

    let salt = generate_salt();
    let commitment = create_commitment(&all_positions, &salt);

    let register_req = RegisterPlayerRequest {
        player_id: "player1".to_string(),
        ships,
        board_commitment: commitment,
        salt,
    };

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/register")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&register_req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_register_player_invalid_commitment() {
    let node = Arc::new(NetworkNode::new("test_node".to_string(), 8080, 10, 2));
    let app = create_router(node.clone());

    let ships = create_valid_fleet();

    let salt = generate_salt();
    // Create commitment with different positions than the ships
    let wrong_positions = vec![(5, 5), (6, 6)];
    let commitment = create_commitment(&wrong_positions, &salt);

    let register_req = RegisterPlayerRequest {
        player_id: "player1".to_string(),
        ships,
        board_commitment: commitment,
        salt,
    };

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/register")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&register_req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_mine_for_shots() {
    let node = Arc::new(NetworkNode::new("test_node".to_string(), 8080, 10, 2));
    
    // Register a player first
    {
        let mut coordinator = node.coordinator.write().await;
        let ships = create_valid_fleet();
        let all_positions: Vec<(u8, u8)> = ships.iter()
            .flat_map(|ship| ship.positions.clone())
            .collect();
        let salt = generate_salt();
        let commitment = create_commitment(&all_positions, &salt);
        coordinator.register_player("player1".to_string(), ships, commitment, salt).unwrap();
    }

    let app = create_router(node.clone());

    let mine_req = MineRequest {
        player_id: "player1".to_string(),
    };

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/mine")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&mine_req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_fire_shot() {
    let node = Arc::new(NetworkNode::new("test_node".to_string(), 8080, 10, 2));
    
    // Register a player and mine for shots
    {
        let mut coordinator = node.coordinator.write().await;
        let ships = create_valid_fleet();
        let all_positions: Vec<(u8, u8)> = ships.iter()
            .flat_map(|ship| ship.positions.clone())
            .collect();
        let salt = generate_salt();
        let commitment = create_commitment(&all_positions, &salt);
        coordinator.register_player("player1".to_string(), ships, commitment, salt).unwrap();
        coordinator.mine_for_shots("player1").unwrap();
    }

    let app = create_router(node.clone());

    let fire_req = FireShotRequest {
        player_id: "player1".to_string(),
        target_x: 5,
        target_y: 5,
    };

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/fire")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&fire_req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_fire_shot_without_shots() {
    let node = Arc::new(NetworkNode::new("test_node".to_string(), 8080, 10, 2));
    
    // Register a player but don't mine for shots
    {
        let mut coordinator = node.coordinator.write().await;
        let ships = create_valid_fleet();
        let all_positions: Vec<(u8, u8)> = ships.iter()
            .flat_map(|ship| ship.positions.clone())
            .collect();
        let salt = generate_salt();
        let commitment = create_commitment(&all_positions, &salt);
        coordinator.register_player("player1".to_string(), ships, commitment, salt).unwrap();
    }

    let app = create_router(node.clone());

    let fire_req = FireShotRequest {
        player_id: "player1".to_string(),
        target_x: 5,
        target_y: 5,
    };

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/fire")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&fire_req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_get_peers() {
    let node = Arc::new(NetworkNode::new("test_node".to_string(), 8080, 10, 2));
    
    // Add some peers
    node.add_peer(Peer::new("localhost".to_string(), 8081)).await;
    node.add_peer(Peer::new("localhost".to_string(), 8082)).await;

    let app = create_router(node.clone());

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/peers")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_add_peer() {
    let node = Arc::new(NetworkNode::new("test_node".to_string(), 8080, 10, 2));
    let app = create_router(node.clone());

    let peer = Peer::new("localhost".to_string(), 8081);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/peers")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&peer).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    
    // Verify peer was added
    let peers = node.get_peers().await;
    assert_eq!(peers.len(), 1);
}

#[tokio::test]
async fn test_get_node_info() {
    let node = Arc::new(NetworkNode::new("test_node".to_string(), 8080, 10, 2));
    let app = create_router(node.clone());

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/info")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_get_game_stats() {
    let node = Arc::new(NetworkNode::new("test_node".to_string(), 8080, 10, 2));
    let app = create_router(node.clone());

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/stats")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_receive_transaction() {
    let node = Arc::new(NetworkNode::new("test_node".to_string(), 8080, 10, 2));
    let app = create_router(node.clone());

    let tx = Transaction::new("player1".to_string(), 5, 5, 0);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/transaction")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&tx).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    
    // Verify transaction was added
    let coordinator = node.coordinator.read().await;
    assert_eq!(coordinator.blockchain.pending_transactions.len(), 1);
}

#[tokio::test]
async fn test_receive_valid_block() {
    let node = Arc::new(NetworkNode::new("test_node".to_string(), 8080, 10, 2));
    
    // Create a valid block
    let mut block = {
        let coordinator = node.coordinator.read().await;
        let latest = coordinator.blockchain.get_latest_block();
        Block::new(1, vec![], latest.hash.clone())
    };
    block.mine(2);

    let app = create_router(node.clone());

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/block")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&block).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_receive_invalid_block_wrong_index() {
    let node = Arc::new(NetworkNode::new("test_node".to_string(), 8080, 10, 2));
    
    // Create a block with wrong index
    let mut block = Block::new(99, vec![], "hash".to_string());
    block.mine(2);

    let app = create_router(node.clone());

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/block")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&block).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_receive_invalid_block_wrong_previous_hash() {
    let node = Arc::new(NetworkNode::new("test_node".to_string(), 8080, 10, 2));
    
    // Create a block with wrong previous hash
    let mut block = Block::new(1, vec![], "wrong_hash".to_string());
    block.mine(2);

    let app = create_router(node.clone());

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/block")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&block).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_sync_blockchain() {
    let node = Arc::new(NetworkNode::new("test_node".to_string(), 8080, 10, 2));
    let app = create_router(node.clone());

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sync")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Should succeed even with no peers
    assert_eq!(response.status(), StatusCode::OK);
}
