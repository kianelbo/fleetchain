use sha2::{Sha256, Digest};
use rand::Rng;
use hex;

/// Generate a random salt for commitment scheme
pub fn generate_salt() -> String {
    let mut rng = rand::thread_rng();
    let salt: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
    hex::encode(salt)
}

/// Create a commitment hash for a board configuration
/// commitment = SHA256(board_positions || salt)
pub fn create_commitment(positions: &[(u8, u8)], salt: &str) -> String {
    let mut hasher = Sha256::new();
    
    // Serialize positions in a deterministic way
    let mut sorted_positions = positions.to_vec();
    sorted_positions.sort();
    
    for (x, y) in sorted_positions {
        hasher.update(&[x, y]);
    }
    
    hasher.update(salt.as_bytes());
    hex::encode(hasher.finalize())
}

/// Verify a commitment against revealed positions and salt
pub fn verify_commitment(
    commitment: &str,
    positions: &[(u8, u8)],
    salt: &str,
) -> bool {
    let calculated = create_commitment(positions, salt);
    calculated == commitment
}

// NOTE: the former toy `HitProof` has been replaced by the real zero-knowledge
// scheme in `crate::zk` (Groth16 over BLS12-381). The SHA-256 commitment helpers
// above are kept for backwards compatibility but are no longer used by the game
// protocol: fleets are now committed with Poseidon inside the proofs.
