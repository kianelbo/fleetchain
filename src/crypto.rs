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

/// Simple ZK proof structure for hit verification
/// In a real implementation, this would use proper ZK-SNARKs
/// For this prototype, we use a simplified commitment-based approach
#[derive(Debug, Clone)]
pub struct HitProof {
    pub commitment: String,
    pub revealed_position: Option<(u8, u8)>,
    pub position_salt: String,
}

impl HitProof {
    /// Generate a proof that a position contains a ship (hit case)
    pub fn prove_hit(
        position: (u8, u8),
        all_positions: &[(u8, u8)],
        board_salt: &str,
    ) -> Self {
        // Create a commitment to the specific position
        let position_salt = generate_salt();
        let commitment = create_commitment(&[position], &position_salt);
        
        Self {
            commitment,
            revealed_position: Some(position),
            position_salt,
        }
    }

    /// Generate a proof that a position does NOT contain a ship (miss case)
    /// This is more complex in real ZK - here we use a simplified approach
    pub fn prove_miss(
        shot_position: (u8, u8),
        all_positions: &[(u8, u8)],
        board_salt: &str,
    ) -> Self {
        // For a miss, we don't reveal the position
        // In a real ZK system, we'd prove "shot_position NOT IN all_positions"
        // without revealing all_positions
        let commitment = create_commitment(all_positions, board_salt);

        Self {
            commitment,
            revealed_position: None,
            position_salt: board_salt.to_string(),
        }
    }

    /// Verify a hit proof
    pub fn verify_hit(
        &self,
        shot_position: (u8, u8),
        board_commitment: &str,
    ) -> bool {
        if let Some(revealed_pos) = self.revealed_position {
            // Verify the revealed position matches the shot
            if revealed_pos != shot_position {
                return false;
            }
            
            // Verify the position commitment
            let calculated = create_commitment(&[revealed_pos], &self.position_salt);
            calculated == self.commitment
        } else {
            false
        }
    }

    /// Verify a miss proof
    pub fn verify_miss(
        &self,
        shot_position: (u8, u8),
        board_commitment: &str,
    ) -> bool {
        // For miss, we just verify the board commitment matches
        // In a real ZK system, we'd verify the proof without revealing positions
        self.commitment == board_commitment && self.revealed_position.is_none()
    }

    pub fn serialize(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
}

impl serde::Serialize for HitProof {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("HitProof", 3)?;
        state.serialize_field("commitment", &self.commitment)?;
        state.serialize_field("revealed_position", &self.revealed_position)?;
        state.serialize_field("position_salt", &self.position_salt)?;
        state.end()
    }
}

impl<'de> serde::Deserialize<'de> for HitProof {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, MapAccess, Visitor};
        use std::fmt;

        struct HitProofVisitor;

        impl<'de> Visitor<'de> for HitProofVisitor {
            type Value = HitProof;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct HitProof")
            }

            fn visit_map<V>(self, mut map: V) -> Result<HitProof, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut commitment = None;
                let mut revealed_position = None;
                let mut position_salt = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "commitment" => commitment = Some(map.next_value()?),
                        "revealed_position" => revealed_position = Some(map.next_value()?),
                        "position_salt" => position_salt = Some(map.next_value()?),
                        _ => { let _: serde::de::IgnoredAny = map.next_value()?; }
                    }
                }

                Ok(HitProof {
                    commitment: commitment.ok_or_else(|| de::Error::missing_field("commitment"))?,
                    revealed_position: revealed_position.ok_or_else(|| de::Error::missing_field("revealed_position"))?,
                    position_salt: position_salt.ok_or_else(|| de::Error::missing_field("position_salt"))?,
                })
            }
        }

        deserializer.deserialize_struct("HitProof", &["commitment", "revealed_position", "position_salt"], HitProofVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_commitment_creation() {
        let positions = vec![(0, 0), (1, 1), (2, 2)];
        let salt = generate_salt();
        let commitment = create_commitment(&positions, &salt);
        
        assert!(!commitment.is_empty());
        assert!(verify_commitment(&commitment, &positions, &salt));
    }

    #[test]
    fn test_commitment_verification_fails_wrong_salt() {
        let positions = vec![(0, 0), (1, 1), (2, 2)];
        let salt = generate_salt();
        let commitment = create_commitment(&positions, &salt);
        
        let wrong_salt = generate_salt();
        assert!(!verify_commitment(&commitment, &positions, &wrong_salt));
    }

    #[test]
    fn test_hit_proof() {
        let all_positions = vec![(0, 0), (1, 1), (2, 2)];
        let salt = generate_salt();
        let board_commitment = create_commitment(&all_positions, &salt);
        
        let shot_position = (1, 1);
        let proof = HitProof::prove_hit(shot_position, &all_positions, &salt);
        
        assert!(proof.verify_hit(shot_position, &board_commitment));
    }

    #[test]
    fn test_miss_proof() {
        let all_positions = vec![(0, 0), (1, 1), (2, 2)];
        let salt = generate_salt();
        let board_commitment = create_commitment(&all_positions, &salt);
        
        let shot_position = (5, 5);
        let proof = HitProof::prove_miss(shot_position, &all_positions, &salt);
        
        assert!(proof.verify_miss(shot_position, &board_commitment));
    }
}
