//! Zero-knowledge hit/miss proofs for FleetChain (Groth16 over BLS12-381).
//!
//! ```text
//!  player (secrets)                          coordinator / any node (public)
//!  ----------------                          -------------------------------
//!  FleetSecret::from_ships(..)
//!  C0 = commit(salt, fleet)    --C0, setup_proof-->  Verifier::verify_setup
//!                                                    stores C0 as player's state
//!                                shot (x, y)  <----  fire_shot
//!  respond(x, y):
//!    hit/alive, C1 = commit(salt', fleet')
//!    shot_proof  --(hit, alive, C1, proof)-->        Verifier::verify_shot(C0, C1, x, y, ..)
//!                                                    state := C1
//! ```
//!
//! The coordinator never sees ship positions, salts, or remaining HP. It only
//! learns, per shot and per defender, one `hit` bit and one `alive` bit, and
//! it accepts a response only if the Groth16 proof shows that bit is
//! consistent with the defender's *currently committed* state.
//!
//! See [`circuits`] for exactly what is proven and [`poseidon`] for the hash.

pub mod circuits;
pub mod client;
pub mod gadgets;
pub mod poseidon;

#[allow(unused_imports)]
pub use circuits::{Segment, SetupCircuit, ShipPlacement, ShotCircuit};
#[allow(unused_imports)]
pub use client::{FleetSecret, PendingResponse};

use bellman::groth16::{
    create_random_proof, generate_random_parameters, prepare_verifying_key, verify_proof, Parameters,
    PreparedVerifyingKey, Proof, VerifyingKey,
};
use bls12_381::{Bls12, Scalar};
use ff::PrimeField;
use rand::rngs::OsRng;
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::sync::{Arc, OnceLock};

/// Ship lengths, fixed and public: Carrier, Cruiser, Submarine, Destroyer.
pub const FLEET_SPEC: [usize; 4] = [4, 3, 2, 1];
/// Total number of ship cells ("segments") in a fleet.
pub const FLEET_CELLS: usize = 10;
/// Coordinates are `u8`, so range checks use 8 bits.
pub const COORD_BITS: usize = 8;
/// Compressed Groth16 proof over BLS12-381: 48 (G1) + 96 (G2) + 48 (G1).
pub const PROOF_BYTES: usize = 192;

const KEYS_MAGIC: &[u8; 8] = b"FLEETZK1";

// ------------------------------------------------------------- encoding ---

pub fn scalar_to_hex(s: &Scalar) -> String {
    hex::encode(s.to_repr())
}

/// Parse a canonical (fully reduced) field element from hex.
pub fn scalar_from_hex(s: &str) -> Result<Scalar, String> {
    let bytes = hex::decode(s).map_err(|_| "Invalid hex encoding".to_string())?;
    let arr: [u8; 32] = bytes.try_into().map_err(|_| "Field element must be 32 bytes".to_string())?;
    Option::<Scalar>::from(Scalar::from_repr(arr)).ok_or_else(|| "Non-canonical field element".to_string())
}

/// serde helper: serialize `Vec<u8>` as a hex string.
pub mod hex_bytes {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &Vec<u8>, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&hex::encode(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let s = String::deserialize(d)?;
        hex::decode(s).map_err(serde::de::Error::custom)
    }
}

fn read_proof(bytes: &[u8]) -> Result<Proof<Bls12>, String> {
    if bytes.len() != PROOF_BYTES {
        return Err(format!("Proof must be exactly {PROOF_BYTES} bytes, got {}", bytes.len()));
    }
    Proof::read(bytes).map_err(|e| format!("Malformed proof: {e}"))
}

pub(crate) fn write_proof(proof: &Proof<Bls12>) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(PROOF_BYTES);
    proof.write(&mut out).map_err(|e| e.to_string())?;
    debug_assert_eq!(out.len(), PROOF_BYTES);
    Ok(out)
}

// ------------------------------------------------------------- verifier ---

/// Everything a node needs to *check* proofs (small; no proving keys).
pub struct Verifier {
    setup_vk: VerifyingKey<Bls12>,
    shot_vk: VerifyingKey<Bls12>,
    setup_pvk: PreparedVerifyingKey<Bls12>,
    shot_pvk: PreparedVerifyingKey<Bls12>,
}

impl Verifier {
    pub fn new(setup_vk: VerifyingKey<Bls12>, shot_vk: VerifyingKey<Bls12>) -> Self {
        Verifier {
            setup_pvk: prepare_verifying_key(&setup_vk),
            shot_pvk: prepare_verifying_key(&shot_vk),
            setup_vk,
            shot_vk,
        }
    }

    /// Check a fleet-setup proof: "`commitment` opens to a legal fleet that
    /// lies inside a `board_size` x `board_size` board".
    pub fn verify_setup(&self, commitment: &Scalar, board_size: u8, proof: &[u8]) -> Result<(), String> {
        let proof = read_proof(proof)?;
        let inputs = [*commitment, circuits::u2s(board_size)];
        verify_proof(&self.setup_pvk, &proof, &inputs).map_err(|_| "Invalid setup proof".to_string())
    }

    /// Check a shot-response proof. The *verifier* supplies `old`, `x`, `y`
    /// from its own records; only `new`, `hit`, `alive` come from the defender.
    #[allow(clippy::too_many_arguments)]
    pub fn verify_shot(
        &self,
        old: &Scalar,
        new: &Scalar,
        x: u8,
        y: u8,
        hit: bool,
        alive: bool,
        proof: &[u8],
    ) -> Result<(), String> {
        let proof = read_proof(proof)?;
        let inputs = [*old, *new, circuits::u2s(x), circuits::u2s(y), circuits::b2s(hit), circuits::b2s(alive)];
        verify_proof(&self.shot_pvk, &proof, &inputs).map_err(|_| "Invalid shot proof".to_string())
    }

    pub fn write<W: Write>(&self, mut w: W) -> std::io::Result<()> {
        w.write_all(KEYS_MAGIC)?;
        w.write_all(&[b'V'])?;
        self.setup_vk.write(&mut w)?;
        self.shot_vk.write(&mut w)
    }

    pub fn read<R: Read>(mut r: R) -> std::io::Result<Self> {
        let mut magic = [0u8; 9];
        r.read_exact(&mut magic)?;
        if &magic[..8] != KEYS_MAGIC || magic[8] != b'V' {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "not a FleetChain verifier-key file"));
        }
        let setup_vk = VerifyingKey::read(&mut r)?;
        let shot_vk = VerifyingKey::read(&mut r)?;
        Ok(Verifier::new(setup_vk, shot_vk))
    }

    /// SHA-256 over both verifying keys. Players and nodes compare this to
    /// confirm they are using the same circuits and trusted setup.
    pub fn fingerprint(&self) -> String {
        let mut h = Sha256::new();
        let mut buf = Vec::new();
        self.setup_vk.write(&mut buf).expect("vec write");
        self.shot_vk.write(&mut buf).expect("vec write");
        h.update(&buf);
        hex::encode(h.finalize())
    }
}

// --------------------------------------------------------------- system ---

/// Proving + verifying keys for both circuits.
pub struct ZkSystem {
    pub setup_params: Parameters<Bls12>,
    pub shot_params: Parameters<Bls12>,
    pub verifier: Arc<Verifier>,
}

impl ZkSystem {
    /// Run a Groth16 setup for both circuits using `rng`.
    ///
    /// **Whoever runs this learns the "toxic waste" and can forge proofs.**
    /// Fine for development and tests; a real deployment needs a multi-party
    /// ceremony (or a transparent proof system).
    pub fn generate<R: RngCore>(rng: &mut R) -> Result<Self, String> {
        let setup_params = generate_random_parameters::<Bls12, _, _>(SetupCircuit::default(), rng)
            .map_err(|e| format!("setup circuit parameter generation failed: {e}"))?;
        let shot_params = generate_random_parameters::<Bls12, _, _>(ShotCircuit::default(), rng)
            .map_err(|e| format!("shot circuit parameter generation failed: {e}"))?;
        let verifier = Arc::new(Verifier::new(setup_params.vk.clone(), shot_params.vk.clone()));
        Ok(ZkSystem { setup_params, shot_params, verifier })
    }

    /// Process-wide development system, generated once on first use.
    /// **INSECURE**: see [`ZkSystem::generate`]. Used by default by
    /// `GameCoordinator::new` and by the tests.
    pub fn insecure_dev_shared() -> &'static ZkSystem {
        static DEV: OnceLock<ZkSystem> = OnceLock::new();
        DEV.get_or_init(|| ZkSystem::generate(&mut OsRng).expect("dev setup"))
    }

    pub fn write<W: Write>(&self, mut w: W) -> std::io::Result<()> {
        w.write_all(KEYS_MAGIC)?;
        w.write_all(&[b'P'])?;
        self.setup_params.write(&mut w)?;
        self.shot_params.write(&mut w)
    }

    pub fn read<R: Read>(mut r: R) -> std::io::Result<Self> {
        let mut magic = [0u8; 9];
        r.read_exact(&mut magic)?;
        if &magic[..8] != KEYS_MAGIC || magic[8] != b'P' {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "not a FleetChain proving-key file"));
        }
        let setup_params = Parameters::read(&mut r, true)?;
        let shot_params = Parameters::read(&mut r, true)?;
        let verifier = Arc::new(Verifier::new(setup_params.vk.clone(), shot_params.vk.clone()));
        Ok(ZkSystem { setup_params, shot_params, verifier })
    }

    pub(crate) fn prove_setup(&self, c: SetupCircuit) -> Result<Vec<u8>, String> {
        let proof = create_random_proof(c, &self.setup_params, &mut OsRng)
            .map_err(|e| format!("setup proving failed: {e}"))?;
        write_proof(&proof)
    }

    pub(crate) fn prove_shot(&self, c: ShotCircuit) -> Result<Vec<u8>, String> {
        let proof = create_random_proof(c, &self.shot_params, &mut OsRng)
            .map_err(|e| format!("shot proving failed: {e}"))?;
        write_proof(&proof)
    }
}
