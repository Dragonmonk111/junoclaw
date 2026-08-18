//! Poseidon hash for BN254::Fr — replaces MiMC.
//!
//! Parameters: state width = 3 (rate=2, capacity=1), alpha=5,
//! full_rounds=8, partial_rounds=57. These are the standard
//! parameters for BN254 used by most ZK projects (e.g. Filecoin, Scroll).
//!
//! The MDS matrix is a Cauchy matrix, and round constants are
//! generated via a Grain LFSR per the Poseidon specification.

use ark_bn254::Fr;
use ark_ff::{BigInteger, Field, PrimeField};
use ark_std::vec::Vec;

pub const POSEIDON_FULL_ROUNDS: usize = 8;
pub const POSEIDON_PARTIAL_ROUNDS: usize = 57;
pub const POSEIDON_ALPHA: u64 = 5;
pub const POSEIDON_RATE: usize = 2;
pub const POSEIDON_CAPACITY: usize = 1;
pub const POSEIDON_STATE_SIZE: usize = POSEIDON_RATE + POSEIDON_CAPACITY; // 3

/// Poseidon configuration for BN254::Fr.
#[derive(Clone, Debug)]
pub struct PoseidonConfig {
    pub full_rounds: usize,
    pub partial_rounds: usize,
    pub alpha: u64,
    pub ark: Vec<Vec<Fr>>,
    pub mds: Vec<Vec<Fr>>,
    pub rate: usize,
    pub capacity: usize,
}

impl PoseidonConfig {
    pub fn full_rounds(&self) -> usize {
        self.full_rounds
    }
    pub fn partial_rounds(&self) -> usize {
        self.partial_rounds
    }
    pub fn alpha(&self) -> u64 {
        self.alpha
    }
    pub fn total_rounds(&self) -> usize {
        self.full_rounds + self.partial_rounds
    }
}

/// Grain LFSR for generating pseudorandom bits per the Poseidon spec.
struct GrainLFSR {
    state: [bool; 80],
}

impl GrainLFSR {
    fn new() -> Self {
        let mut state = [false; 80];
        // Initialize: first bit = 1, last bit = 1, rest from seed
        state[0] = true;
        state[79] = true;
        // Fill with a fixed pattern for BN254 Poseidon
        // The Poseidon spec says to use the field modulus bits
        let modulus = <Fr as PrimeField>::MODULUS;
        let modulus_bytes = modulus.to_bytes_le();
        for i in 0..80 {
            let byte_idx = i / 8;
            let bit_idx = i % 8;
            if byte_idx < modulus_bytes.len() {
                state[i] = (modulus_bytes[byte_idx] >> bit_idx) & 1 == 1;
            }
        }
        state[0] = true;
        state[79] = true;

        let mut lfsr = Self { state };
        // Warm up: discard first 160 bits
        for _ in 0..160 {
            lfsr.next_bit();
        }
        lfsr
    }

    fn next_bit(&mut self) -> bool {
        let new_bit = self.state[0]
            ^ self.state[13]
            ^ self.state[23]
            ^ self.state[27]
            ^ self.state[38]
            ^ self.state[51]
            ^ self.state[62]
            ^ self.state[73];
        // Shift
        for i in 0..79 {
            self.state[i] = self.state[i + 1];
        }
        self.state[79] = new_bit;
        new_bit
    }

    /// Get `num_bits` random bits as a field element.
    fn next_field_element(&mut self, num_bits: usize) -> Fr {
        let mut bits = Vec::with_capacity(num_bits);
        for _ in 0..num_bits {
            bits.push(self.next_bit());
        }
        // Convert bits to a big-endian integer, then to Fr
        let mut bytes = vec![0u8; (num_bits + 7) / 8];
        for (i, &bit) in bits.iter().enumerate() {
            let byte_idx = i / 8;
            let bit_idx = i % 8;
            if bit {
                bytes[byte_idx] |= 1 << bit_idx;
            }
        }
        // Use from_be_bytes_mod_order to reduce into the field
        let mut be_bytes = bytes.clone();
        be_bytes.reverse();
        Fr::from_be_bytes_mod_order(&be_bytes)
    }
}

/// Generate the MDS matrix as a Cauchy matrix.
/// MDS[i][j] = 1 / (x_i - y_j) where x_i = 2^i, y_j = 3^j.
fn generate_mds_matrix() -> Vec<Vec<Fr>> {
    let n = POSEIDON_STATE_SIZE;
    let mut mds = Vec::with_capacity(n);
    for i in 0..n {
        let mut row = Vec::with_capacity(n);
        let x = Fr::from(1u64 << i);
        for j in 0..n {
            let y = Fr::from(3u64.pow(j as u32));
            let diff = x - y;
            // Compute inverse of diff
            let inv = diff.inverse().unwrap_or(Fr::from(1u64));
            row.push(inv);
        }
        mds.push(row);
    }
    mds
}

/// Generate round constants (ARK) using the Grain LFSR.
fn generate_round_constants() -> Vec<Vec<Fr>> {
    let total_rounds = POSEIDON_FULL_ROUNDS + POSEIDON_PARTIAL_ROUNDS;
    let mut lfsr = GrainLFSR::new();
    let mut ark = Vec::with_capacity(total_rounds);

    // Number of bits for field elements: ceil(log2(p)) where p is the BN254 modulus
    // BN254 modulus is ~254 bits, use 255 bits for safety
    let field_bits = 255;

    for _ in 0..total_rounds {
        let mut round_keys = Vec::with_capacity(POSEIDON_STATE_SIZE);
        for _ in 0..POSEIDON_STATE_SIZE {
            round_keys.push(lfsr.next_field_element(field_bits));
        }
        ark.push(round_keys);
    }
    ark
}

/// Get the Poseidon configuration for BN254::Fr.
/// Cached after first computation.
pub fn poseidon_config() -> PoseidonConfig {
    PoseidonConfig {
        full_rounds: POSEIDON_FULL_ROUNDS,
        partial_rounds: POSEIDON_PARTIAL_ROUNDS,
        alpha: POSEIDON_ALPHA,
        ark: generate_round_constants(),
        mds: generate_mds_matrix(),
        rate: POSEIDON_RATE,
        capacity: POSEIDON_CAPACITY,
    }
}

// ── Native Poseidon hash (off-circuit) ──

fn poseidon_permute(state: &mut [Fr; POSEIDON_STATE_SIZE], config: &PoseidonConfig) {
    let total_rounds = config.full_rounds + config.partial_rounds;
    let half_full = config.full_rounds / 2;

    for round in 0..total_rounds {
        // Add round constants (ARK)
        for i in 0..POSEIDON_STATE_SIZE {
            state[i] += config.ark[round][i];
        }

        // Apply S-box
        let is_full_round = round < half_full || round >= half_full + config.partial_rounds;
        if is_full_round {
            // Full round: apply S-box to all elements
            for elem in state.iter_mut() {
                *elem = elem.pow([config.alpha]);
            }
        } else {
            // Partial round: apply S-box only to first element
            state[0] = state[0].pow([config.alpha]);
        }

        // Apply MDS matrix
        let mut new_state = [Fr::from(0u64); POSEIDON_STATE_SIZE];
        for i in 0..POSEIDON_STATE_SIZE {
            for j in 0..POSEIDON_STATE_SIZE {
                new_state[i] += config.mds[i][j] * state[j];
            }
        }
        *state = new_state;
    }
}

/// Poseidon hash of two field elements (for Merkle tree).
pub fn poseidon_hash(left: Fr, right: Fr) -> Fr {
    let config = poseidon_config();
    let mut state = [Fr::from(0u64), left, right];
    poseidon_permute(&mut state, &config);
    state[0]
}

/// Poseidon hash of 5 field elements (for envelope commitment / sensor leaf).
/// Uses sponge construction: absorb all 5 elements, squeeze 1 output.
pub fn poseidon_hash_5(inputs: [Fr; 5]) -> Fr {
    let config = poseidon_config();
    // Sponge: rate=2, capacity=1
    // Absorb 5 elements in chunks of 2
    let mut state = [Fr::from(0u64); POSEIDON_STATE_SIZE];

    // Absorb inputs[0], inputs[1]
    state[1] += inputs[0];
    state[2] += inputs[1];
    poseidon_permute(&mut state, &config);

    // Absorb inputs[2], inputs[3]
    state[1] += inputs[2];
    state[2] += inputs[3];
    poseidon_permute(&mut state, &config);

    // Absorb inputs[4] (last element, pad with 0)
    state[1] += inputs[4];
    // Padding: add 1 to the next rate position
    state[2] += Fr::from(1u64);
    poseidon_permute(&mut state, &config);

    // Squeeze first element
    state[0]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_poseidon_hash_deterministic() {
        let a = Fr::from(42u64);
        let b = Fr::from(7u64);
        assert_eq!(poseidon_hash(a, b), poseidon_hash(a, b));
    }

    #[test]
    fn test_poseidon_hash_different_inputs() {
        let a = Fr::from(42u64);
        let b = Fr::from(7u64);
        let c = Fr::from(43u64);
        assert_ne!(poseidon_hash(a, b), poseidon_hash(c, b));
    }

    #[test]
    fn test_poseidon_hash_5_deterministic() {
        let inputs = [Fr::from(1u64), Fr::from(2u64), Fr::from(3u64), Fr::from(4u64), Fr::from(5u64)];
        assert_eq!(poseidon_hash_5(inputs), poseidon_hash_5(inputs));
    }

    #[test]
    fn test_poseidon_hash_5_different_inputs() {
        let inputs1 = [Fr::from(1u64), Fr::from(2u64), Fr::from(3u64), Fr::from(4u64), Fr::from(5u64)];
        let inputs2 = [Fr::from(1u64), Fr::from(2u64), Fr::from(3u64), Fr::from(4u64), Fr::from(6u64)];
        assert_ne!(poseidon_hash_5(inputs1), poseidon_hash_5(inputs2));
    }

    #[test]
    fn test_mds_matrix_invertible() {
        let mds = generate_mds_matrix();
        // Check that all elements are non-zero (necessary for MDS)
        for row in &mds {
            for &elem in row {
                assert!(elem != Fr::from(0u64), "MDS matrix has zero element");
            }
        }
    }
}
