#![allow(dead_code)] // the binary re-declares the library modules wholesale
mod api;
mod blockchain;
mod coordinator;
mod crypto;
mod game;
mod network;
mod zk;

use clap::Parser;
use network::{NetworkNode, Peer};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use zk::{Verifier, ZkSystem};

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

    /// Path to blockchain data file (default: ./data/{node_id}_blockchain.json)
    #[arg(long)]
    blockchain_path: Option<String>,

    /// Groth16 proving-key file; the verifying keys live next to it with a
    /// `.vk` suffix. Created (INSECURE single-party setup) if missing. All
    /// nodes and players of a game must use the same keys.
    #[arg(long, default_value = "./data/zk_params.bin")]
    zk_params: String,

    /// Eliminate players who leave a shot unanswered for more than this many
    /// blocks (disabled if omitted)
    #[arg(long)]
    response_timeout_blocks: Option<u64>,
}

fn vk_path(params: &Path) -> PathBuf {
    let mut s = params.as_os_str().to_owned();
    s.push(".vk");
    PathBuf::from(s)
}

/// Make sure key files exist (running a dev setup if not) and return the verifier.
fn ensure_zk_keys(params: &Path) -> Arc<Verifier> {
    let vk = vk_path(params);
    if let Ok(f) = std::fs::File::open(&vk) {
        match Verifier::read(std::io::BufReader::new(f)) {
            Ok(v) => return Arc::new(v),
            Err(e) => panic!("Cannot read verifying keys {:?}: {}", vk, e),
        }
    }
    if let Some(dir) = params.parent() {
        std::fs::create_dir_all(dir).expect("Failed to create zk params directory");
    }
    let sys = if params.exists() {
        println!("Deriving verifying keys from {:?} ...", params);
        load_proving_system(params)
    } else {
        eprintln!("WARNING: generating ZK parameters with a single-party setup.");
        eprintln!("         Whoever runs this can forge proofs. Development use only.");
        let sys = ZkSystem::generate(&mut rand::rngs::OsRng).expect("ZK setup failed");
        let f = std::fs::File::create(params).expect("cannot write proving keys");
        sys.write(std::io::BufWriter::new(f))
            .expect("cannot write proving keys");
        println!("✓ Wrote proving keys to {:?}", params);
        sys
    };
    let f = std::fs::File::create(&vk).expect("cannot write verifying keys");
    sys.verifier
        .write(std::io::BufWriter::new(f))
        .expect("cannot write verifying keys");
    println!("✓ Wrote verifying keys to {:?}", vk);
    sys.verifier.clone()
}

fn load_proving_system(params: &Path) -> ZkSystem {
    let f =
        std::fs::File::open(params).unwrap_or_else(|e| panic!("Cannot open {:?}: {}", params, e));
    ZkSystem::read(std::io::BufReader::new(f))
        .unwrap_or_else(|e| panic!("Cannot read {:?}: {}", params, e))
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    println!("=== FleetChain: Blockchain Battleship ===");
    println!("Node ID: {}", args.node_id);
    println!("Port: {}", args.port);
    println!("Grid Size: {}x{}", args.grid_size, args.grid_size);
    println!("Mining Difficulty: {}\n", args.difficulty);

    // Determine blockchain path
    let blockchain_path = if let Some(path) = args.blockchain_path {
        PathBuf::from(path)
    } else {
        // Create data directory if it doesn't exist
        let data_dir = PathBuf::from("./data");
        if !data_dir.exists() {
            std::fs::create_dir_all(&data_dir).expect("Failed to create data directory");
        }
        data_dir.join(format!("{}_blockchain.json", args.node_id))
    };

    println!("Blockchain file: {:?}\n", blockchain_path);

    // Create network node with persistence
    let node = Arc::new(NetworkNode::with_persistence(
        args.node_id.clone(),
        args.port,
        args.grid_size,
        args.difficulty,
        blockchain_path,
    ));

    // Load (or create) the ZK verifying keys and hand them to the coordinator
    let zk_params = PathBuf::from(&args.zk_params);
    let verifier = ensure_zk_keys(&zk_params);
    println!("ZK verifying-key fingerprint: {}\n", verifier.fingerprint());
    {
        let mut coordinator = node.coordinator.write().await;
        coordinator.set_verifier(verifier);
        coordinator.response_timeout_blocks = args.response_timeout_blocks;
    }

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
        let sys = load_proving_system(&zk_params);
        run_demo(node.clone(), &sys).await;
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
    println!("  POST /api/respond        - Answer a shot with a ZK proof");
    println!("  POST /api/pending        - Shots a player still owes a proof for");
    println!("  GET  /api/results        - Public shot log and verified responses");
    println!("  GET  /api/zk             - Verifying-key fingerprint");
    println!("  POST /api/mine           - Mine for shots");
    println!("  POST /api/shots          - Get shot balance");
    println!("  POST /api/peers          - Add peer");
    println!("  POST /api/sync           - Sync blockchain\n");

    let app = api::create_router(node);
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", args.port))
        .await
        .unwrap();

    axum::serve(listener, app).await.unwrap();
}

async fn run_demo(node: Arc<NetworkNode>, sys: &ZkSystem) {
    let mut coordinator = node.coordinator.write().await;
    demo_game(&mut coordinator, sys);
}

/// Register a player from a secret fleet: the coordinator only ever sees the
/// commitment and the proof.
fn demo_register(
    game: &mut coordinator::GameCoordinator,
    sys: &ZkSystem,
    id: &str,
    ships: Vec<game::Ship>,
) -> Option<zk::FleetSecret> {
    let secret = match zk::FleetSecret::from_ships(&ships, game.grid.size) {
        Ok(s) => s,
        Err(e) => {
            println!("✗ Invalid fleet for {}: {}", id, e);
            return None;
        }
    };
    let proof = secret.prove_setup(sys).expect("setup proving");
    match game.register_player(id.to_string(), secret.commitment_hex(), &proof) {
        Ok(_) => {
            println!(
                "✓ {} registered. Public commitment: {}...",
                id,
                &secret.commitment_hex()[..16]
            );
            Some(secret)
        }
        Err(e) => {
            println!("✗ Failed to register {}: {}", id, e);
            None
        }
    }
}

/// Answer every shot `id` still owes, printing what becomes public.
fn demo_respond(
    game: &mut coordinator::GameCoordinator,
    sys: &ZkSystem,
    id: &str,
    secret: &mut zk::FleetSecret,
) {
    for shot in game.pending_shots(id) {
        let pending = match secret.respond(sys, id, &shot) {
            Ok(p) => p,
            Err(e) => {
                println!("✗ {} could not prove: {}", id, e);
                return;
            }
        };
        match game.report_hit(pending.report.clone()) {
            Ok(out) => {
                println!(
                    "  {} answers shot #{} ({}, {}) from {}: {}{}",
                    id,
                    out.shot_index,
                    shot.x,
                    shot.y,
                    shot.shooter,
                    if out.is_hit { "HIT" } else { "miss" },
                    if out.defeated {
                        "  -- fleet destroyed!"
                    } else {
                        ""
                    }
                );
                secret.apply(pending);
            }
            Err(e) => println!("✗ {} response rejected: {}", id, e),
        }
    }
}

fn demo_game(game: &mut coordinator::GameCoordinator, sys: &ZkSystem) {
    use game::Ship;

    println!("--- Registration (commitment + zero-knowledge fleet proof) ---");
    let mut p1 = demo_register(
        game,
        sys,
        "player1",
        vec![
            Ship::new("carrier".to_string(), vec![(0, 0), (0, 1), (0, 2), (0, 3)]),
            Ship::new("cruiser".to_string(), vec![(2, 0), (2, 1), (2, 2)]),
            Ship::new("submarine".to_string(), vec![(4, 0), (4, 1)]),
            Ship::new("destroyer".to_string(), vec![(6, 0)]),
        ],
    );
    let mut p2 = demo_register(
        game,
        sys,
        "player2",
        vec![
            Ship::new("carrier".to_string(), vec![(5, 5), (5, 6), (5, 7), (5, 8)]),
            Ship::new("cruiser".to_string(), vec![(7, 5), (7, 6), (7, 7)]),
            Ship::new("submarine".to_string(), vec![(9, 5), (9, 6)]),
            Ship::new("destroyer".to_string(), vec![(0, 9)]),
        ],
    );
    let (Some(p1), Some(p2)) = (p1.as_mut(), p2.as_mut()) else {
        return;
    };

    println!("\n{}", game.get_stats());

    println!("\n--- Mining Phase ---");
    for id in ["player1", "player2"] {
        match game.mine_for_shots(id) {
            Ok(shots) => println!("✓ {} earned {} shot(s)", id, shots),
            Err(e) => println!("✗ Mining failed: {}", e),
        }
    }

    println!("\n--- Combat Phase ---");
    println!("player1 fires at (5, 5), a cell where player2 hides a ship...");
    match game.fire_shot("player1".to_string(), 5, 5) {
        Ok(_) => println!("✓ Shot recorded; player2 now owes a proof"),
        Err(e) => println!("✗ Shot failed: {}", e),
    }
    println!("player2 fires at (3, 3), an empty cell...");
    match game.fire_shot("player2".to_string(), 3, 3) {
        Ok(_) => println!("✓ Shot recorded; player1 now owes a proof"),
        Err(e) => println!("✗ Shot failed: {}", e),
    }

    println!("\n--- Zero-knowledge responses ---");
    demo_respond(game, sys, "player2", p2);
    demo_respond(game, sys, "player1", p1);
    println!(
        "(Secretly: player1 has {} live cells, player2 has {}. The coordinator only knows the bits above.)",
        p1.remaining_cells(),
        p2.remaining_cells()
    );

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
        println!(
            "Block #{}: {} transactions, hash: {}...",
            i,
            block.transactions.len(),
            &block.hash[..16]
        );
    }
}
