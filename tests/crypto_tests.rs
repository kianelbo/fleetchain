use fleetchain::crypto::{generate_salt, create_commitment, verify_commitment};

#[test]
fn test_salt_generation() {
    let salt1 = generate_salt();
    let salt2 = generate_salt();
    
    // Salts should be different
    assert_ne!(salt1, salt2);
    
    // Salt should be non-empty
    assert!(!salt1.is_empty());
    assert!(!salt2.is_empty());
}

#[test]
fn test_salt_length() {
    let salt = generate_salt();
    // SHA-256 hash in hex should be 64 characters
    assert_eq!(salt.len(), 64);
}

#[test]
fn test_commitment_creation() {
    let positions = vec![(0, 0), (0, 1), (0, 2)];
    let salt = generate_salt();
    
    let commitment = create_commitment(&positions, &salt);
    
    assert!(!commitment.is_empty());
    assert_eq!(commitment.len(), 64); // SHA-256 hex
}

#[test]
fn test_commitment_deterministic() {
    let positions = vec![(0, 0), (0, 1), (0, 2)];
    let salt = "test_salt".to_string();
    
    let commitment1 = create_commitment(&positions, &salt);
    let commitment2 = create_commitment(&positions, &salt);
    
    // Same input should produce same commitment
    assert_eq!(commitment1, commitment2);
}

#[test]
fn test_commitment_different_positions() {
    let positions1 = vec![(0, 0), (0, 1)];
    let positions2 = vec![(0, 0), (0, 2)];
    let salt = "test_salt".to_string();
    
    let commitment1 = create_commitment(&positions1, &salt);
    let commitment2 = create_commitment(&positions2, &salt);
    
    // Different positions should produce different commitments
    assert_ne!(commitment1, commitment2);
}

#[test]
fn test_commitment_different_salt() {
    let positions = vec![(0, 0), (0, 1)];
    let salt1 = "salt1".to_string();
    let salt2 = "salt2".to_string();
    
    let commitment1 = create_commitment(&positions, &salt1);
    let commitment2 = create_commitment(&positions, &salt2);
    
    // Different salts should produce different commitments
    assert_ne!(commitment1, commitment2);
}

#[test]
fn test_verify_commitment_valid() {
    let positions = vec![(0, 0), (0, 1), (0, 2)];
    let salt = generate_salt();
    let commitment = create_commitment(&positions, &salt);
    
    assert!(verify_commitment(&commitment, &positions, &salt));
}

#[test]
fn test_verify_commitment_invalid_positions() {
    let positions = vec![(0, 0), (0, 1)];
    let wrong_positions = vec![(0, 0), (0, 2)];
    let salt = generate_salt();
    let commitment = create_commitment(&positions, &salt);
    
    assert!(!verify_commitment(&commitment, &wrong_positions, &salt));
}

#[test]
fn test_verify_commitment_invalid_salt() {
    let positions = vec![(0, 0), (0, 1)];
    let salt = generate_salt();
    let wrong_salt = generate_salt();
    let commitment = create_commitment(&positions, &salt);
    
    assert!(!verify_commitment(&commitment, &positions, &wrong_salt));
}

#[test]
fn test_verify_commitment_invalid_commitment() {
    let positions = vec![(0, 0), (0, 1)];
    let salt = generate_salt();
    let wrong_commitment = "invalid_commitment_hash".to_string();
    
    assert!(!verify_commitment(&wrong_commitment, &positions, &salt));
}

#[test]
fn test_commitment_order_independence() {
    let positions1 = vec![(0, 0), (0, 1), (0, 2)];
    let positions2 = vec![(0, 2), (0, 0), (0, 1)];
    let salt = "test_salt".to_string();
    
    let commitment1 = create_commitment(&positions1, &salt);
    let commitment2 = create_commitment(&positions2, &salt);
    
    // Commitments should be the same regardless of order (positions are sorted internally)
    assert_eq!(commitment1, commitment2);
}

#[test]
fn test_commitment_empty_positions() {
    let positions: Vec<(u8, u8)> = vec![];
    let salt = generate_salt();
    
    let commitment = create_commitment(&positions, &salt);
    assert!(!commitment.is_empty());
}

#[test]
fn test_commitment_single_position() {
    let positions = vec![(5, 5)];
    let salt = generate_salt();
    
    let commitment = create_commitment(&positions, &salt);
    assert!(verify_commitment(&commitment, &positions, &salt));
}

#[test]
fn test_commitment_large_positions() {
    let positions: Vec<(u8, u8)> = (0..100).map(|i| (i % 10, i / 10)).collect();
    let salt = generate_salt();
    
    let commitment = create_commitment(&positions, &salt);
    assert!(verify_commitment(&commitment, &positions, &salt));
}

#[test]
fn test_commitment_duplicate_positions() {
    let positions = vec![(0, 0), (0, 0), (0, 1)];
    let salt = generate_salt();
    
    let commitment = create_commitment(&positions, &salt);
    assert!(verify_commitment(&commitment, &positions, &salt));
}

#[test]
fn test_commitment_boundary_positions() {
    let positions = vec![(0, 0), (255, 255), (128, 128)];
    let salt = generate_salt();
    
    let commitment = create_commitment(&positions, &salt);
    assert!(verify_commitment(&commitment, &positions, &salt));
}

#[test]
fn test_multiple_commitments_unique() {
    let mut commitments = std::collections::HashSet::new();
    
    for i in 0..10 {
        let positions = vec![(i, 0), (i, 1)];
        let salt = generate_salt();
        let commitment = create_commitment(&positions, &salt);
        commitments.insert(commitment);
    }
    
    // All commitments should be unique
    assert_eq!(commitments.len(), 10);
}

#[test]
fn test_commitment_collision_resistance() {
    let positions1 = vec![(0, 0), (0, 1)];
    let positions2 = vec![(1, 0), (1, 1)];
    let salt = generate_salt();
    
    let commitment1 = create_commitment(&positions1, &salt);
    let commitment2 = create_commitment(&positions2, &salt);
    
    // Different positions should produce different commitments
    assert_ne!(commitment1, commitment2);
}

#[test]
fn test_salt_randomness() {
    let mut salts = std::collections::HashSet::new();
    
    for _ in 0..100 {
        let salt = generate_salt();
        salts.insert(salt);
    }
    
    // All salts should be unique (extremely high probability)
    assert_eq!(salts.len(), 100);
}

#[test]
fn test_commitment_with_special_characters_in_salt() {
    let positions = vec![(0, 0), (0, 1)];
    let salt = "!@#$%^&*()_+-=[]{}|;':\",./<>?".to_string();
    
    let commitment = create_commitment(&positions, &salt);
    assert!(verify_commitment(&commitment, &positions, &salt));
}

#[test]
fn test_commitment_with_unicode_salt() {
    let positions = vec![(0, 0), (0, 1)];
    let salt = "你好世界🌍🚀".to_string();
    
    let commitment = create_commitment(&positions, &salt);
    assert!(verify_commitment(&commitment, &positions, &salt));
}

#[test]
fn test_commitment_with_empty_salt() {
    let positions = vec![(0, 0), (0, 1)];
    let salt = "".to_string();
    
    let commitment = create_commitment(&positions, &salt);
    assert!(verify_commitment(&commitment, &positions, &salt));
}

#[test]
fn test_commitment_with_very_long_salt() {
    let positions = vec![(0, 0), (0, 1)];
    let salt = "a".repeat(10000);
    
    let commitment = create_commitment(&positions, &salt);
    assert!(verify_commitment(&commitment, &positions, &salt));
}

#[test]
fn test_verify_commitment_case_sensitive() {
    let positions = vec![(0, 0), (0, 1)];
    let salt = "TestSalt".to_string();
    let commitment = create_commitment(&positions, &salt);
    
    let wrong_salt = "testsalt".to_string();
    assert!(!verify_commitment(&commitment, &positions, &wrong_salt));
}

#[test]
fn test_commitment_hex_format() {
    let positions = vec![(0, 0), (0, 1)];
    let salt = generate_salt();
    let commitment = create_commitment(&positions, &salt);
    
    // Check that commitment is valid hex
    assert!(commitment.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn test_commitment_reproducibility() {
    let positions = vec![(5, 5), (6, 6), (7, 7)];
    let salt = "reproducible_salt".to_string();
    
    // Create commitment multiple times
    let commitments: Vec<String> = (0..10)
        .map(|_| create_commitment(&positions, &salt))
        .collect();
    
    // All should be identical
    assert!(commitments.windows(2).all(|w| w[0] == w[1]));
}

#[test]
fn test_commitment_avalanche_effect() {
    let positions1 = vec![(0, 0), (0, 1)];
    let positions2 = vec![(0, 0), (0, 2)]; // Only one position different
    let salt = "test_salt".to_string();
    
    let commitment1 = create_commitment(&positions1, &salt);
    let commitment2 = create_commitment(&positions2, &salt);
    
    // Small change in input should produce very different output
    let different_chars = commitment1.chars()
        .zip(commitment2.chars())
        .filter(|(a, b)| a != b)
        .count();
    
    // At least half the characters should be different (avalanche effect)
    assert!(different_chars > commitment1.len() / 2);
}

#[test]
fn test_verify_commitment_comprehensive() {
    // Test a realistic game scenario
    let carrier_positions = vec![(0, 0), (0, 1), (0, 2), (0, 3), (0, 4)];
    let battleship_positions = vec![(2, 0), (2, 1), (2, 2), (2, 3)];
    let destroyer_positions = vec![(4, 0), (4, 1), (4, 2)];
    
    let mut all_positions = Vec::new();
    all_positions.extend(carrier_positions);
    all_positions.extend(battleship_positions);
    all_positions.extend(destroyer_positions);
    
    let salt = generate_salt();
    let commitment = create_commitment(&all_positions, &salt);
    
    // Verify with correct data
    assert!(verify_commitment(&commitment, &all_positions, &salt));
    
    // Verify fails with wrong data
    let mut wrong_positions = all_positions.clone();
    wrong_positions[0] = (9, 9);
    assert!(!verify_commitment(&commitment, &wrong_positions, &salt));
}
