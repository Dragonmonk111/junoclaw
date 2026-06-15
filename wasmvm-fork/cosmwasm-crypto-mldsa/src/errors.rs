//! Error types for ML-DSA host-function failures.

use core::fmt;

/// ML-DSA host-function error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum MlDsaError {
    /// Input (public key or signature) has an unexpected length for the
    /// requested variant.
    InvalidInputLength {
        /// Parameter-set name (e.g. "ML-DSA-44").
        variant: &'static str,
        /// Which field was wrong (e.g. "public_key" or "signature").
        field: &'static str,
        /// Expected byte length for this field.
        expected: usize,
        /// Actual byte length received.
        actual: usize,
    },
    /// The supplied variant code is not supported (valid codes: 44, 65, 87).
    UnknownVariant(u32),
    /// The public-key bytes failed to decode into a valid ML-DSA public key.
    InvalidPublicKey,
    /// The signature is cryptographically invalid. Reserved: `ml_dsa_verify`
    /// returns `Ok(false)` for a well-formed but invalid signature; this
    /// variant exists for callers that prefer an error path.
    InvalidSignature,
}

impl fmt::Display for MlDsaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MlDsaError::InvalidInputLength {
                variant,
                field,
                expected,
                actual,
            } => write!(
                f,
                "ml-dsa: invalid {field} length for {variant}: expected {expected}, got {actual}"
            ),
            MlDsaError::UnknownVariant(v) => write!(f, "ml-dsa: unknown variant code {v}"),
            MlDsaError::InvalidPublicKey => write!(f, "ml-dsa: invalid public key encoding"),
            MlDsaError::InvalidSignature => write!(f, "ml-dsa: signature verification failed"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for MlDsaError {}
