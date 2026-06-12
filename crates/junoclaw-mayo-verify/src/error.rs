//! Error types for MAYO verification failures.

use core::fmt;

/// MAYO verifier error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// Input has wrong length for the parameter set.
    InvalidLength {
        expected: usize,
        actual: usize,
    },
    /// Verification failed (invalid signature).
    VerifyFailed,
    /// AES-128-CTR key expansion failed.
    AesError,
    /// SHAKE256 operation failed.
    ShakeError,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InvalidLength { expected, actual } => {
                write!(f, "invalid length: expected {}, got {}", expected, actual)
            }
            Error::VerifyFailed => write!(f, "signature verification failed"),
            Error::AesError => write!(f, "AES operation failed"),
            Error::ShakeError => write!(f, "SHAKE256 operation failed"),
        }
    }
}

/// Convenience alias for `Result<T, Error>`.
pub type Result<T> = core::result::Result<T, Error>;

#[cfg(feature = "std")]
impl std::error::Error for Error {}
