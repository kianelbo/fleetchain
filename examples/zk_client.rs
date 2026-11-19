//! Minimal player client for a FleetChain node.
//!
//! Your fleet and salt live ONLY in a local secret file (plaintext JSON; keep
//! it private). The node only ever receives commitments, result bits and proofs.
//!
//!     # register a random fleet (needs the proving-key file the node created)
//!     cargo run --example zk_client -- register --id alice
//!     # answer every shot you owe a proof for
//!     cargo run --example zk_client -- respond  --id alice
//!     # look at your own secret state
//!     cargo run --example zk_client -- status   --id alice

use clap::{Parser, Subcommand};
use fleetchain::game::ShotRecord;
use fleetchain::network::{ApiResponse, PendingShotsRequest, RegisterPlayerRequest};
use fleetchain::zk::{FleetSecret, ZkSystem};
use serde_json::Value;
use std::path::PathBuf;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
    /// Player id (required)
    #[arg(long, global = true)]
    id: Option<String>,
    /// Node base URL
    #[arg(long, global = true, default_value = "http://localhost:8080")]
    node: String,
    /// Proving-key file (created by the node on first start)
    #[arg(long, global = true, default_value = "./data/zk_params.bin")]
    params: PathBuf,
    /// Secret file (default ./data/<id>.secret.json)
    #[arg(long, global = true)]
    secret: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Create a random fleet and register its commitment + proof
    Register {
        #[arg(long, default_value_t = 10)]
        board: u8,
    },
    /// Prove and submit answers to every shot this player owes
    Respond,
    /// Print this player's own (secret) state
    Status,
}

fn load_system(p: &PathBuf) -> Result<ZkSystem, String> {
    let f = std::fs::File::open(p).map_err(|e| format!("cannot open {p:?}: {e}"))?;
    ZkSystem::read(std::io::BufReader::new(f)).map_err(|e| format!("cannot read {p:?}: {e}"))
}

fn save_secret(path: &PathBuf, s: &FleetSecret) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, s.to_json()).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let mut cli = Cli::parse();
    let id = cli.id.take().ok_or("--id is required")?;
    cli.id = Some(id.clone());
    let secret_path = cli.secret.clone().unwrap_or_else(|| PathBuf::from(format!("./data/{id}.secret.json")));
    let http = reqwest::Client::new();
    let post = |path: &str| http.post(format!("{}{}", cli.node, path));

    match cli.cmd {
        Cmd::Register { board } => {
            if secret_path.exists() {
                return Err(format!("{secret_path:?} already exists; refusing to overwrite a secret fleet"));
            }
            let sys = load_system(&cli.params)?;
            let fleet = FleetSecret::random(board)?;
            let proof = fleet.prove_setup(&sys)?;
            let req = RegisterPlayerRequest {
                player_id: id.clone(),
                board_commitment: fleet.commitment_hex(),
                setup_proof: proof,
            };
            let resp: ApiResponse<String> = post("/api/register").json(&req).send().await.map_err(|e| e.to_string())?
                .json().await.map_err(|e| e.to_string())?;
            if !resp.success {
                return Err(format!("registration rejected: {}", resp.error.unwrap_or_default()));
            }
            // Only keep the secret once the node accepted the registration.
            save_secret(&secret_path, &fleet)?;
            println!("registered {} (commitment {}...); secret saved to {:?}", id, &fleet.commitment_hex()[..16], secret_path);
        }
        Cmd::Respond => {
            let sys = load_system(&cli.params)?;
            let mut fleet = FleetSecret::from_json(&std::fs::read_to_string(&secret_path).map_err(|e| e.to_string())?)?;
            let resp: ApiResponse<Vec<ShotRecord>> = post("/api/pending")
                .json(&PendingShotsRequest { player_id: id.clone() }).send().await.map_err(|e| e.to_string())?
                .json().await.map_err(|e| e.to_string())?;
            let shots = resp.data.ok_or_else(|| resp.error.unwrap_or_default())?;
            if shots.is_empty() {
                println!("nothing to answer");
            }
            for shot in shots {
                let pending = fleet.respond(&sys, &id, &shot)?;
                let r: Value = post("/api/respond").json(&pending.report).send().await.map_err(|e| e.to_string())?
                    .json().await.map_err(|e| e.to_string())?;
                if r["success"] != true {
                    return Err(format!("shot #{} rejected: {}", shot.index, r["error"]));
                }
                println!(
                    "answered shot #{} at ({}, {}): {}{}",
                    shot.index, shot.x, shot.y,
                    if pending.report.is_hit { "HIT" } else { "miss" },
                    if pending.report.is_alive { "" } else { " -- fleet destroyed" }
                );
                fleet.apply(pending);
                save_secret(&secret_path, &fleet)?; // persist after every accepted step
            }
        }
        Cmd::Status => {
            let fleet = FleetSecret::from_json(&std::fs::read_to_string(&secret_path).map_err(|e| e.to_string())?)?;
            println!("{fleet:?}");
        }
    }
    Ok(())
}
