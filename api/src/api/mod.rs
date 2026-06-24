//! Route handlers for the coral-server REST API.
//!
//! Each sub-module owns one resource group and exposes a `routes()` function
//! that returns a [`Router`](axum::Router) ready to be nested under the versioned prefix.

pub mod agents;
pub mod coralos;
pub mod messaging;
pub mod pay_demo;
pub mod shared_state;
pub mod solana_pay;
pub mod weather;
pub mod workflows;
