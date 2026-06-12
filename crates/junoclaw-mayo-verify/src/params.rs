//! MAYO parameter sets (NIST Round 4).

/// Trait for MAYO parameter sets.
pub trait ParameterSet: Copy + core::fmt::Debug + PartialEq + Eq {
    /// Human-readable name.
    const NAME: &'static str;
    /// Number of variables (n).
    const N: usize;
    /// Number of equations (m).
    const M: usize;
    /// Oil space dimension (o).
    const O: usize;
    /// Vinegar space dimension (v = n - o).
    const V: usize = Self::N - Self::O;
    /// Number of signature vectors (k).
    const K: usize;
    /// Field size (q = 16).
    const Q: usize = 16;
    /// m-vector limbs (ceil(m / 16)).
    const M_VEC_LIMBS: usize = (Self::M + 15) / 16;
    /// Bytes to encode m nibbles.
    const M_BYTES: usize = (Self::M + 1) / 2;
    /// Bytes for O matrix (v * o nibbles).
    const O_BYTES: usize = (Self::V * Self::O + 1) / 2;
    /// Bytes for vinegar vectors.
    const V_BYTES: usize = (Self::V + 1) / 2;
    /// R bytes.
    const R_BYTES: usize = (Self::K * Self::O + 1) / 2;
    /// P1 bytes (upper-triangular v×v matrix of m-vectors).
    const P1_BYTES: usize = Self::V * (Self::V + 1) / 2 * Self::M_VEC_LIMBS * 8;
    /// P2 bytes (v×o matrix of m-vectors).
    const P2_BYTES: usize = Self::V * Self::O * Self::M_VEC_LIMBS * 8;
    /// P3 bytes (upper-triangular o×o matrix of m-vectors).
    const P3_BYTES: usize = Self::O * (Self::O + 1) / 2 * Self::M_VEC_LIMBS * 8;
    /// Compact secret key bytes.
    const SK_BYTES: usize;
    /// Compact public key bytes.
    const PK_BYTES: usize;
    /// Signature bytes.
    const SIG_BYTES: usize;
    /// Salt bytes.
    const SALT_BYTES: usize;
    /// Digest bytes.
    const DIGEST_BYTES: usize;
    /// Public key seed bytes.
    const PK_SEED_BYTES: usize;
    /// Secret key seed bytes.
    const SK_SEED_BYTES: usize;
    /// Expanded public key limbs.
    const EPK_LIMBS: usize = Self::P1_BYTES / 8 + Self::P2_BYTES / 8 + Self::P3_BYTES / 8;
    /// f(z) tail coefficients for field reduction.
    const F_TAIL: [u8; 4];
    /// A_cols = k * o + 1.
    const A_COLS: usize = Self::K * Self::O + 1;

    /// Verify a MAYO signature with this parameter set.
    fn verify(message: &[u8], signature: &[u8], cpk: &[u8]) -> crate::error::Result<bool> {
        crate::verify::verify::<Self>(message, signature, cpk)
    }
}

/// MAYO-1 parameter set (NIST Level 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mayo1;

impl ParameterSet for Mayo1 {
    const NAME: &'static str = "MAYO-1";
    const N: usize = 86;
    const M: usize = 78;
    const O: usize = 8;
    const K: usize = 10;
    const SK_BYTES: usize = 24;
    const PK_BYTES: usize = 1420;
    const SIG_BYTES: usize = 454;
    const SALT_BYTES: usize = 24;
    const DIGEST_BYTES: usize = 32;
    const PK_SEED_BYTES: usize = 16;
    const SK_SEED_BYTES: usize = 24;
    // f(z) = z^78 + z^2 + z + x^3 — distinct from the m=64 polynomial.
    const F_TAIL: [u8; 4] = [8, 1, 1, 0];
}

/// MAYO-2 parameter set (NIST Level 1, smallest signatures — recommended for on-chain).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mayo2;

impl ParameterSet for Mayo2 {
    const NAME: &'static str = "MAYO-2";
    const N: usize = 81;
    const M: usize = 64;
    const O: usize = 17;
    const K: usize = 4;
    const SK_BYTES: usize = 24;
    const PK_BYTES: usize = 4912;
    const SIG_BYTES: usize = 186;
    const SALT_BYTES: usize = 24;
    const DIGEST_BYTES: usize = 32;
    const PK_SEED_BYTES: usize = 16;
    const SK_SEED_BYTES: usize = 24;
    const F_TAIL: [u8; 4] = [8, 0, 2, 8];
}

/// MAYO-3 parameter set (NIST Level 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mayo3;

impl ParameterSet for Mayo3 {
    const NAME: &'static str = "MAYO-3";
    const N: usize = 118;
    const M: usize = 108;
    const O: usize = 10;
    const K: usize = 11;
    const SK_BYTES: usize = 32;
    const PK_BYTES: usize = 2986;
    const SIG_BYTES: usize = 681;
    const SALT_BYTES: usize = 32;
    const DIGEST_BYTES: usize = 48;
    const PK_SEED_BYTES: usize = 16;
    const SK_SEED_BYTES: usize = 32;
    const F_TAIL: [u8; 4] = [8, 0, 1, 7];
}

/// MAYO-5 parameter set (NIST Level 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mayo5;

impl ParameterSet for Mayo5 {
    const NAME: &'static str = "MAYO-5";
    const N: usize = 154;
    const M: usize = 142;
    const O: usize = 12;
    const K: usize = 12;
    const SK_BYTES: usize = 40;
    const PK_BYTES: usize = 5554;
    const SIG_BYTES: usize = 964;
    const SALT_BYTES: usize = 40;
    const DIGEST_BYTES: usize = 64;
    const PK_SEED_BYTES: usize = 16;
    const SK_SEED_BYTES: usize = 40;
    const F_TAIL: [u8; 4] = [4, 0, 8, 1];
}
