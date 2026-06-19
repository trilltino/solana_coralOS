//! Jito MEV bundle integration — stub for hackathon demo.
//!
//! This module provides the types and client surface for Jito block-engine
//! bundles. Full gRPC implementation is gated behind a feature flag.

/// Placeholder Jito client.
pub struct JitoClient;

impl Default for JitoClient {
    fn default() -> Self {
        Self::new()
    }
}

impl JitoClient {
    pub fn new() -> Self {
        Self
    }
}
