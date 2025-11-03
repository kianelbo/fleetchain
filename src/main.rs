mod blockchain;
mod game;
mod crypto;
mod coordinator;
mod network;
mod api;

use clap::Parser;
use network::{NetworkNode, Peer};
use std::sync::Arc;

/// FleetChain: Distributed Blockchain Battleship
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Port to run the node on
    #[arg(short, long, default_value_t = 8080)]
    port: u16,

    /// Node ID (unique identifier for this node)
    #[arg(short, long, default_value = "node1")]
    node_id: String,

    /// Grid size for the battleship game
    #[arg(short, long, default_value_t = 10)]
    grid_size: u8,

    /// Mining difficulty (number of leading zeros)
    #[arg(short, long, default_value_t = 2)]
    difficulty: usize,

    /// Peer addresses to connect to (format: host:port)
    #[arg(long, value_delimiter = ',')]
    peers: Vec<String>,

    /// Run in demo mode (single node with test game)
    #[arg(long)]
    demo: bool,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    println!("=== FleetChain: Blockchain Battleship ===");
    println!("Node ID: {}", args.node_id);
    println!("Port: {}", args.port);
    println!("Grid Size: {}x{}", args.grid_size, args.grid_size);
    println!("Mining Difficulty: {}\n", args.difficulty);

    // Create network node
    let node = Arc::new(NetworkNode::new(
        args.node_id.clone(),
        args.port,
        args.grid_size,
        args.difficulty,
    ));

    // Connect to peers
    if !args.peers.is_empty() {
        println!("Connecting to peers...");
        for peer_addr in &args.peers {
            if let Some((host, port_str)) = peer_addr.split_once(':') {
                if let Ok(port) = port_str.parse::<u16>() {
                    let peer = Peer::new(host.to_string(), port);
                    node.add_peer(peer.clone()).await;
                    println!("✓ Added peer: {}", peer.url());
                    
                    // Announce ourselves to the peer
                    if let Err(e) = node.announce_to_peer(&peer).await {
                        eprintln!("✗ Failed to announce to peer: {}", e);
                    }
                }
            }
        }

        // Sync blockchain with network
        println!("\nSynchronizing blockchain...");
        if let Err(e) = node.sync_with_network().await {
            eprintln!("✗ Sync failed: {}", e);
        } else {
            println!("✓ Blockchain synchronized");
        }
    }

    if args.demo {
        // Run demo mode
        println!("\n=== Running Demo Mode ===\n");
        run_demo(node.clone()).await;
    }

    // Start HTTP server
    println!("\n=== Starting HTTP Server ===");
    println!("Listening on http://localhost:{}", args.port);
    println!("\nAvailable endpoints:");
    println!("  GET  /api/info           - Node information");
    println!("  GET  /api/stats          - Game statistics");
    println!("  GET  /api/blockchain     - Full blockchain");
    println!("  GET  /api/peers          - Connected peers");
    println!("  POST /api/register       - Register player");
    println!("  POST /api/fire           - Fire shot");
    println!("  POST /api/mine           - Mine for shots");
    println!("  POST /api/peers          - Add peer");
    println!("  POST /api/sync           - Sync blockchain\n");

    let app = api::create_router(node);
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", args.port))
        .await
        .unwrap();

    axum::serve(listener, app).await.unwrap();
}

async fn run_demo(node: Arc<NetworkNode>) {
    let mut coordinator = node.coordinator.write().await;
    demo_game(&mut coordinator);
}

fn demo_game(game: &mut coordinator::GameCoordinator) {
    use game::Ship;
    use crypto::{generate_salt, create_commitment};
    // Player 1 setup
    println!("Registering Player 1...");
    let player1_ships = vec![
        Ship::new("carrier".to_string(), vec![(0, 0), (0, 1), (0, 2), (0, 3), (0, 4)]),
        Ship::new("battleship".to_string(), vec![(2, 0), (2, 1), (2, 2), (2, 3)]),
        Ship::new("destroyer".to_string(), vec![(4, 0), (4, 1), (4, 2)]),
    ];

    let player1_positions: Vec<(u8, u8)> = player1_ships.iter()
        .flat_map(|ship| ship.positions.clone())
        .collect();

    let player1_salt = generate_salt();
    let player1_commitment = create_commitment(&player1_positions, &player1_salt);

    match game.register_player(
        "player1".to_string(),
        player1_ships,
        player1_commitment.clone(),
        player1_salt.clone(),
    ) {
        Ok(_) => println!("✓ Player 1 registered with commitment: {}...", &player1_commitment[..16]),
        Err(e) => println!("✗ Failed to register Player 1: {}", e),
    }

    // Player 2 setup
    println!("\nRegistering Player 2...");
    let player2_ships = vec![
        Ship::new("carrier".to_string(), vec![(5, 5), (5, 6), (5, 7), (5, 8), (5, 9)]),
        Ship::new("battleship".to_string(), vec![(7, 5), (7, 6), (7, 7), (7, 8)]),
        Ship::new("destroyer".to_string(), vec![(9, 5), (9, 6), (9, 7)]),
    ];

    let player2_positions: Vec<(u8, u8)> = player2_ships.iter()
        .flat_map(|ship| ship.positions.clone())
        .collect();

    let player2_salt = generate_salt();
    let player2_commitment = create_commitment(&player2_positions, &player2_salt);

    match game.register_player(
        "player2".to_string(),
        player2_ships,
        player2_commitment.clone(),
        player2_salt.clone(),
    ) {
        Ok(_) => println!("✓ Player 2 registered with commitment: {}...", &player2_commitment[..16]),
        Err(e) => println!("✗ Failed to register Player 2: {}", e),
    }

    // Display initial stats
    println!("\n{}", game.get_stats());

    println!("\n--- Mining Phase ---");
    println!("Player 1 mining for shots...");
    match game.mine_for_shots("player1") {
        Ok(shots) => println!("✓ Player 1 earned {} shot(s)", shots),
        Err(e) => println!("✗ Mining failed: {}", e),
    }

    println!("Player 2 mining for shots...");
    match game.mine_for_shots("player2") {
        Ok(shots) => println!("✓ Player 2 earned {} shot(s)", shots),
        Err(e) => println!("✗ Mining failed: {}", e),
    }

    // Shooting demonstration
    println!("\n--- Combat Phase ---");
    println!("Player 1 fires at (5, 5)...");
    match game.fire_shot("player1".to_string(), 5, 5) {
        Ok(_) => println!("✓ Shot fired! Transaction added to blockchain"),
        Err(e) => println!("✗ Shot failed: {}", e),
    }

    println!("Player 2 fires at (0, 0)...");
    match game.fire_shot("player2".to_string(), 0, 0) {
        Ok(_) => println!("✓ Shot fired! Transaction added to blockchain"),
        Err(e) => println!("✗ Shot failed: {}", e),
    }

    // Mine the transactions
    println!("\nMining combat transactions...");
    game.mine_for_shots("player1").ok();

    // Display final stats
    println!("\n--- Final Stats ---");
    println!("{}", game.get_stats());
    println!("Blockchain valid: {}", game.verify_blockchain());
    
    // Display blockchain
    println!("\n--- Blockchain ---");
    for (i, block) in game.blockchain.chain.iter().enumerate() {
        println!("Block #{}: {} transactions, hash: {}...", 
            i, block.transactions.len(), &block.hash[..16]);
    }
}

#[cfg(test)]
mod tests {
    use crate::coordinator::GameCoordinator;

    #[test]
    fn test_game_initialization() {
        let game = GameCoordinator::new(10, 2);
        assert_eq!(game.grid.size, 10);
        assert_eq!(game.blockchain.difficulty, 2);
    }
}
