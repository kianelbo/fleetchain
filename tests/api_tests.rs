use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::util::ServiceExt;
use std::sync::Arc;
use fleetchain::api::create_router;
use fleetchain::network::{NetworkNode, RegisterPlayerRequest, FireShotRequest, MineRequest, PendingShotsRequest, Peer};
use fleetchain::game::ShotRecord;
use fleetchain::zk::FleetSecret;
use fleetchain::game::Ship;
mod common;
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

async fn post_json(app: axum::Router, uri: &str, body: String) -> (StatusCode, serde_json::Value) {
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null))
}

#[tokio::test]
async fn test_register_player() {
    let node = Arc::new(NetworkNode::new("test_node".to_string(), 8080, 10, 2));
    let app = create_router(node.clone());

    let secret = FleetSecret::from_ships(&create_valid_fleet(), 10).unwrap();
    let register_req = RegisterPlayerRequest {
        player_id: "player1".to_string(),
        board_commitment: secret.commitment_hex(),
        setup_proof: secret.prove_setup(common::sys()).unwrap(),
    };

    let (status, _) = post_json(app, "/api/register", serde_json::to_string(&register_req).unwrap()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(node.coordinator.read().await.players.len(), 1);
}

#[tokio::test]
async fn test_register_player_invalid_commitment() {
    let node = Arc::new(NetworkNode::new("test_node".to_string(), 8080, 10, 2));
    let app = create_router(node.clone());

    // A valid proof for one fleet, submitted with the commitment of another.
    let secret = FleetSecret::from_ships(&create_valid_fleet(), 10).unwrap();
    let other = FleetSecret::from_ships(&create_valid_fleet(), 10).unwrap();
    let register_req = RegisterPlayerRequest {
        player_id: "player1".to_string(),
        board_commitment: other.commitment_hex(),
        setup_proof: secret.prove_setup(common::sys()).unwrap(),
    };

    let (status, _) = post_json(app, "/api/register", serde_json::to_string(&register_req).unwrap()).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(node.coordinator.read().await.players.len(), 0);
}

#[tokio::test]
async fn test_register_request_never_contains_ship_positions() {
    // The wire format has no field that could carry a fleet or a salt.
    let secret = FleetSecret::from_ships(&create_valid_fleet(), 10).unwrap();
    let req = RegisterPlayerRequest {
        player_id: "player1".to_string(),
        board_commitment: secret.commitment_hex(),
        setup_proof: secret.prove_setup(common::sys()).unwrap(),
    };
    let json: serde_json::Value = serde_json::to_value(&req).unwrap();
    let mut keys: Vec<_> = json.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    assert_eq!(keys, ["board_commitment", "player_id", "setup_proof"]);
}

#[tokio::test]
async fn test_zero_knowledge_round_trip_over_http() {
    let node = Arc::new(NetworkNode::new("test_node".to_string(), 8080, 10, 2));
    let sys = common::sys();

    let mut defender = FleetSecret::from_ships(&create_valid_fleet(), 10).unwrap();
    let attacker = FleetSecret::from_ships(&common::horizontal_fleet(), 10).unwrap();
    for (id, s) in [("defender", &defender), ("attacker", &attacker)] {
        let req = RegisterPlayerRequest {
            player_id: id.to_string(),
            board_commitment: s.commitment_hex(),
            setup_proof: s.prove_setup(sys).unwrap(),
        };
        let (status, _) = post_json(create_router(node.clone()), "/api/register", serde_json::to_string(&req).unwrap()).await;
        assert_eq!(status, StatusCode::OK);
    }

    // attacker fires at the defender's carrier (0, 1) -- a hit
    let (status, _) = post_json(
        create_router(node.clone()),
        "/api/fire",
        serde_json::to_string(&FireShotRequest { player_id: "attacker".into(), target_x: 0, target_y: 1 }).unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // the defender asks what it owes ...
    let (status, body) = post_json(
        create_router(node.clone()),
        "/api/pending",
        serde_json::to_string(&PendingShotsRequest { player_id: "defender".into() }).unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let pending: Vec<ShotRecord> = serde_json::from_value(body["data"].clone()).unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!((pending[0].x, pending[0].y), (0, 1));

    // ... the shooter owes nothing for its own shot
    let (_, body) = post_json(
        create_router(node.clone()),
        "/api/pending",
        serde_json::to_string(&PendingShotsRequest { player_id: "attacker".into() }).unwrap(),
    )
    .await;
    assert_eq!(body["data"].as_array().unwrap().len(), 0);

    // ... and answers with a proof
    let pr = defender.respond(sys, "defender", &pending[0]).unwrap();
    let (status, body) = post_json(create_router(node.clone()), "/api/respond", serde_json::to_string(&pr.report).unwrap()).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["is_hit"], true);
    defender.apply(pr);

    // The same answer cannot be submitted twice.
    let mut replay = defender.respond(sys, "defender", &pending[0]).unwrap().report;
    replay.shot_index = 0;
    let (status, _) = post_json(create_router(node.clone()), "/api/respond", serde_json::to_string(&replay).unwrap()).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Public ledger shows only the result bits.
    let resp = create_router(node.clone())
        .oneshot(Request::builder().uri("/api/results").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(text.contains("\"is_hit\":true"));

    // Verifying-key fingerprint is exposed so players can check they agree.
    let resp = create_router(node.clone())
        .oneshot(Request::builder().uri("/api/zk").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_mine_for_shots() {
    let node = Arc::new(NetworkNode::new("test_node".to_string(), 8080, 10, 2));
    
    // Register a player first
    {
        let mut coordinator = node.coordinator.write().await;
        let ships = create_valid_fleet();
        common::register(&mut coordinator, &"player1".to_string(), &ships).unwrap();
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
        common::register(&mut coordinator, &"player1".to_string(), &ships).unwrap();
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
        common::register(&mut coordinator, &"player1".to_string(), &ships).unwrap();
    }

    let app = create_router(node.clone());

    // First fire should succeed because registration grants 1 shot UTXO
    let fire_req1 = FireShotRequest {
        player_id: "player1".to_string(),
        target_x: 5,
        target_y: 5,
    };

    let response1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/fire")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&fire_req1).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response1.status(), StatusCode::OK);

    // Second fire without mining should fail (no UTXOs left)
    let fire_req2 = FireShotRequest {
        player_id: "player1".to_string(),
        target_x: 6,
        target_y: 6,
    };

    let response2 = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/fire")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&fire_req2).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response2.status(), StatusCode::BAD_REQUEST);
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
