//! Error types for MAYO host-function failures.

use core::fmt;

/// MAYO host-function error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum MayoError {
    /// Input (PK, message, or signature) has an unexpected length for the
    /// requested variant.
    InvalidInputLength {
        /// Parameter-set name (e.g. "MAYO-2").
        variant: &'static str,
        /// Which field was wrong (e.g. "public_key" or "signature").
        field: &'static str,
        /// Expected byte length for this field.
        expected: usize,
        /// Actual byte length received.
        actual: usize,
    },
    /// The supplied variant code is not supported.
    UnknownVariant(u32),
    /// The signature is invalid (cryptographic verification failed).
    InvalidSignature,
    /// An internal error occurred during verification (e.g. AES or SHAKE
    /// failure). These should not happen with well-formed inputs.
    InternalError,
}

impl fmt::Display for MayoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MayoError::InvalidInputLength {
                variant,
                field,
                expected,
                actual,
            } => write!(
                f,
                "mayo: invalid {field} length for {variant}: expected {expected}, got {actual}"
            ),
            MayoError::UnknownVariant(v) => write!(f, "mayo: unknown variant code {v}"),
            MayoError::InvalidSignature => write!(f, "mayo: signature verification failed"),
            MayoError::InternalError => write!(f, "mayo: internal verification error"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for MayoError {}

impl From<junoclaw_mayo_verify::Error> for MayoError {
    fn from(e: junoclaw_mayo_verify::Error) -> Self {
        match e {
            junoclaw_mayo_verify::Error::InvalidLength { .. } => MayoError::InvalidInputLength {
                variant: "(unknown)",
                field: "input",
                expected: 0,
                actual: 0,
            },
            junoclaw_mayo_verify::Error::VerifyFailed => MayoError::InvalidSignature,
            junoclaw_mayo_verify::Error::AesError => MayoError::InternalError,
            junoclaw_mayo_verify::Error::ShakeError => MayoError::InternalError,
        }
    }
}
