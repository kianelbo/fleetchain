use fleetchain::crypto::{generate_salt, create_commitment, HitProof};

/// Demonstrates the zero-knowledge proof system for hit/miss verification
fn main() {
    println!("=== Zero-Knowledge Proof Demonstration ===\n");
    
    // Setup: Player has ships at these positions
    let ship_positions = vec![
        (0, 0), (0, 1), (0, 2),  // Ship 1
        (5, 5), (5, 6), (5, 7),  // Ship 2
    ];
    
    let salt = generate_salt();
    let board_commitment = create_commitment(&ship_positions, &salt);
    
    println!("Player's board commitment: {}...", &board_commitment[..32]);
    println!("Ship positions are HIDDEN from other players\n");
    
    // Scenario 1: Opponent shoots at (0, 1) - HIT
    println!("--- Scenario 1: HIT ---");
    let shot_1 = (0, 1);
    println!("Opponent fires at ({}, {})", shot_1.0, shot_1.1);
    
    let hit_proof = HitProof::prove_hit(shot_1, &ship_positions, &salt);
    println!("Player generates HIT proof");
    println!("  - Proof commitment: {}...", &hit_proof.commitment[..32]);
    println!("  - Revealed position: {:?}", hit_proof.revealed_position);
    
    let is_valid = hit_proof.verify_hit(shot_1, &board_commitment);
    println!("Proof verification: {}", if is_valid { "✓ VALID" } else { "✗ INVALID" });
    println!("The proof confirms a ship exists at ({}, {}) without revealing other ships\n", shot_1.0, shot_1.1);
    
    // Scenario 2: Opponent shoots at (3, 3) - MISS
    println!("--- Scenario 2: MISS ---");
    let shot_2 = (3, 3);
    println!("Opponent fires at ({}, {})", shot_2.0, shot_2.1);
    
    let miss_proof = HitProof::prove_miss(shot_2, &ship_positions, &salt);
    println!("Player generates MISS proof");
    println!("  - Proof commitment: {}...", &miss_proof.commitment[..32]);
    println!("  - Revealed position: {:?}", miss_proof.revealed_position);
    
    let is_valid = miss_proof.verify_miss(shot_2, &board_commitment);
    println!("Proof verification: {}", if is_valid { "✓ VALID" } else { "✗ INVALID" });
    println!("The proof confirms NO ship at ({}, {}) without revealing ship locations\n", shot_2.0, shot_2.1);
    
    // Scenario 3: Cheating attempt - False HIT claim
    println!("--- Scenario 3: CHEATING ATTEMPT ---");
    let shot_3 = (9, 9);
    println!("Opponent fires at ({}, {})", shot_3.0, shot_3.1);
    println!("Player tries to claim HIT (but there's no ship there)");
    
    // Try to create a false hit proof
    let false_hit_proof = HitProof::prove_hit(shot_3, &ship_positions, &salt);
    let is_valid = false_hit_proof.verify_hit(shot_3, &board_commitment);
    println!("Proof verification: {}", if is_valid { "✓ VALID" } else { "✗ INVALID - Cheating detected!" });
    
    // Scenario 4: Verify commitment at game end
    println!("\n--- Scenario 4: GAME END VERIFICATION ---");
    println!("At game end, player reveals salt and positions");
    println!("Verifying original commitment...");
    
    use fleetchain::crypto::verify_commitment;
    let commitment_valid = verify_commitment(&board_commitment, &ship_positions, &salt);
    println!("Commitment verification: {}", if commitment_valid { "✓ VALID" } else { "✗ INVALID" });
    println!("This proves the player didn't change their ship positions during the game");
    
    println!("\n=== Key Properties ===");
    println!("✓ Privacy: Ship positions remain hidden until revealed");
    println!("✓ Verifiability: Hit/miss claims can be verified");
    println!("✓ Non-repudiation: Players can't change their commitments");
    println!("✓ Cheat-proof: False claims are detected by verification");
}
