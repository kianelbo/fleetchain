use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use std::sync::Arc;
use tower_http::cors::CorsLayer;

use crate::blockchain::{Block, Transaction};
use crate::network::{
    NetworkNode, RegisterPlayerRequest, FireShotRequest, MineRequest,
    ApiResponse, Peer, BlockchainInfo, NodeInfo,
};

/// Application state shared across handlers
pub type AppState = Arc<NetworkNode>;

/// Create the API router with all endpoints
pub fn create_router(state: AppState) -> Router {
    Router::new()
        // Blockchain endpoints
        .route("/api/blockchain", get(get_blockchain))
        .route("/api/block", post(receive_block))
        .route("/api/transaction", post(receive_transaction))
        
        // Game endpoints
        .route("/api/register", post(register_player))
        .route("/api/fire", post(fire_shot))
        .route("/api/mine", post(mine_for_shots))
        
        // Network endpoints
        .route("/api/peers", get(get_peers))
        .route("/api/peers", post(add_peer))
        .route("/api/sync", post(sync_blockchain))
        
        // Info endpoints
        .route("/api/info", get(get_node_info))
        .route("/api/stats", get(get_game_stats))
        
        .layer(CorsLayer::permissive())
        .with_state(state)
}

/// Get the entire blockchain
async fn get_blockchain(
    State(node): State<AppState>,
) -> Json<crate::blockchain::Blockchain> {
    let coordinator = node.coordinator.read().await;
    Json(coordinator.blockchain.clone())
}

/// Receive a new block from a peer
async fn receive_block(
    State(node): State<AppState>,
    Json(block): Json<Block>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let mut coordinator = node.coordinator.write().await;
    
    // Validate the block
    if block.index != coordinator.blockchain.chain.len() as u64 {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error("Invalid block index".to_string())),
        );
    }

    if block.previous_hash != coordinator.blockchain.get_latest_block().hash {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error("Invalid previous hash".to_string())),
        );
    }

    if !block.hash.starts_with(&"0".repeat(coordinator.blockchain.difficulty)) {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error("Invalid proof of work".to_string())),
        );
    }

    // Add the block to our chain
    coordinator.blockchain.chain.push(block);
    coordinator.blockchain.pending_transactions.clear();

    (
        StatusCode::OK,
        Json(ApiResponse::success("Block accepted".to_string())),
    )
}

/// Receive a new transaction from a peer
async fn receive_transaction(
    State(node): State<AppState>,
    Json(transaction): Json<Transaction>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let mut coordinator = node.coordinator.write().await;
    coordinator.blockchain.add_transaction(transaction);

    (
        StatusCode::OK,
        Json(ApiResponse::success("Transaction accepted".to_string())),
    )
}

/// Register a new player
async fn register_player(
    State(node): State<AppState>,
    Json(req): Json<RegisterPlayerRequest>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let mut coordinator = node.coordinator.write().await;

    match coordinator.register_player(
        req.player_id.clone(),
        req.ships,
        req.board_commitment,
        req.salt,
    ) {
        Ok(_) => (
            StatusCode::OK,
            Json(ApiResponse::success(format!("Player {} registered", req.player_id))),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error(e)),
        ),
    }
}

/// Fire a shot
async fn fire_shot(
    State(node): State<AppState>,
    Json(req): Json<FireShotRequest>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let mut coordinator = node.coordinator.write().await;

    match coordinator.fire_shot(req.player_id.clone(), req.target_x, req.target_y) {
        Ok(_) => {
            // Get the transaction that was just added
            if let Some(transaction) = coordinator.blockchain.pending_transactions.last() {
                let tx = transaction.clone();
                drop(coordinator); // Release the lock before broadcasting
                
                // Broadcast to peers
                let _ = node.broadcast_transaction(&tx).await;
                
                (
                    StatusCode::OK,
                    Json(ApiResponse::success("Shot fired and broadcasted".to_string())),
                )
            } else {
                (
                    StatusCode::OK,
                    Json(ApiResponse::success("Shot fired".to_string())),
                )
            }
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error(e)),
        ),
    }
}

/// Mine for shots
async fn mine_for_shots(
    State(node): State<AppState>,
    Json(req): Json<MineRequest>,
) -> (StatusCode, Json<ApiResponse<u32>>) {
    let mut coordinator = node.coordinator.write().await;

    match coordinator.mine_for_shots(&req.player_id) {
        Ok(shots) => {
            // Get the newly mined block
            if let Some(block) = coordinator.blockchain.chain.last() {
                let new_block = block.clone();
                drop(coordinator); // Release the lock before broadcasting
                
                // Broadcast the new block to peers
                let _ = node.broadcast_block(&new_block).await;
                
                (
                    StatusCode::OK,
                    Json(ApiResponse::success(shots)),
                )
            } else {
                (
                    StatusCode::OK,
                    Json(ApiResponse::success(shots)),
                )
            }
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error(e)),
        ),
    }
}

/// Get all connected peers
async fn get_peers(
    State(node): State<AppState>,
) -> Json<Vec<Peer>> {
    let peers = node.get_peers().await;
    Json(peers)
}

/// Add a new peer
async fn add_peer(
    State(node): State<AppState>,
    Json(peer): Json<Peer>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    node.add_peer(peer.clone()).await;
    
    (
        StatusCode::OK,
        Json(ApiResponse::success(format!("Peer {} added", peer.url()))),
    )
}

/// Synchronize blockchain with all peers
async fn sync_blockchain(
    State(node): State<AppState>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match node.sync_with_network().await {
        Ok(_) => (
            StatusCode::OK,
            Json(ApiResponse::success("Blockchain synchronized".to_string())),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::error(e)),
        ),
    }
}

/// Get node information
async fn get_node_info(
    State(node): State<AppState>,
) -> Json<NodeInfo> {
    let coordinator = node.coordinator.read().await;
    let peers = node.get_peers().await;

    let blockchain_info = BlockchainInfo {
        length: coordinator.blockchain.chain.len(),
        difficulty: coordinator.blockchain.difficulty,
        pending_transactions: coordinator.blockchain.pending_transactions.len(),
        is_valid: coordinator.blockchain.is_chain_valid(),
    };

    Json(NodeInfo {
        node_id: node.node_id.clone(),
        port: node.port,
        peers_count: peers.len(),
        blockchain_info,
    })
}

/// Get game statistics
async fn get_game_stats(
    State(node): State<AppState>,
) -> (StatusCode, Json<crate::coordinator::GameStats>) {
    let coordinator = node.coordinator.read().await;
    (StatusCode::OK, Json(coordinator.get_stats()))
}
