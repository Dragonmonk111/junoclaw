//! JunoClaw x402 gateway — library surface.
//!
//! Exposed for integration tests and downstream embedders. Most operators
//! consume the `junoclaw-x402-gateway` binary directly; the library exists
//! so the routes / x402 envelope spec / cosmos client wrapper can be reused
//! (e.g. by a future `junoclaw-cli x402-relay` subcommand).

pub mod config;
pub mod cosmos;
pub mod error;
pub mod routes;
pub mod x402;
