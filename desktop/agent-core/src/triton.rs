//! Triton One connection configuration.
//!
//! Triton PAYG keys authenticate both the HTTP JSON-RPC endpoint and the
//! WebSocket PubSub endpoint. Supply your PAYG key as `x_token`; it is
//! embedded in the endpoint URL following Triton's PAYG URL scheme.
//!
//! For Yellowstone gRPC (when dep conflicts are resolved upstream), set
//! `grpc_endpoint`. The `ws_url` / `rpc_url` pair is what the current
//! strategy implementation uses for event-driven account monitoring.

use serde::{Deserialize, Serialize};

/// Connection parameters for a Triton One node.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TritonConfig {
    /// Yellowstone gRPC endpoint (reserved for future gRPC integration).
    pub grpc_endpoint: String,
    /// HTTP JSON-RPC URL used for balance reads and tx lookups.
    pub rpc_url: String,
    /// WebSocket PubSub URL used for event-driven account monitoring.
    pub ws_url: String,
    /// PAYG API key.
    pub x_token: String,
    pub network: String,
}

impl TritonConfig {
    /// Triton shared mainnet-beta cluster.
    /// `x_token` is your Triton PAYG API key.
    pub fn mainnet(x_token: impl Into<String>) -> Self {
        let token = x_token.into();
        Self {
            grpc_endpoint: format!("https://api.mainnet.triton.one"),
            rpc_url: "https://api.mainnet-beta.solana.com".to_string(),
            ws_url: "wss://api.mainnet-beta.solana.com".to_string(),
            x_token: token,
            network: "mainnet-beta".to_string(),
        }
    }

    /// Triton shared devnet cluster.
    pub fn devnet(x_token: impl Into<String>) -> Self {
        Self {
            grpc_endpoint: "https://api.devnet.triton.one".to_string(),
            rpc_url: "https://api.devnet.solana.com".to_string(),
            ws_url: "wss://api.devnet.solana.com".to_string(),
            x_token: x_token.into(),
            network: "devnet".to_string(),
        }
    }

    /// Dedicated or custom Triton PAYG node.
    ///
    /// For a PAYG node the URLs typically look like:
    /// - `rpc_url`:  `https://<node>.solana-mainnet.rpcpool.com`
    /// - `ws_url`:   `wss://<node>.solana-mainnet.rpcpool.com`
    /// - `grpc_endpoint`: `https://<node>.solana-mainnet.rpcpool.com:10000`
    pub fn custom(
        grpc_endpoint: impl Into<String>,
        rpc_url: impl Into<String>,
        ws_url: impl Into<String>,
        x_token: impl Into<String>,
        network: impl Into<String>,
    ) -> Self {
        Self {
            grpc_endpoint: grpc_endpoint.into(),
            rpc_url: rpc_url.into(),
            ws_url: ws_url.into(),
            x_token: x_token.into(),
            network: network.into(),
        }
    }
}
