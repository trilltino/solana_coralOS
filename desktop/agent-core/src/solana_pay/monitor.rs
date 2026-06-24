use async_trait::async_trait;
use chrono::Utc;
use futures::StreamExt;
use solana_client::nonblocking::pubsub_client::PubsubClient;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_client::rpc_config::RpcAccountInfoConfig;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::agent::{AgentAction, AgentState};
use crate::solana_pay::url::{encode_transfer_url, TransferUrlFields};
use crate::strategy::Strategy;
use crate::triton::TritonConfig;

/// Agent strategy that opens a persistent Solana WebSocket subscription to
/// `recipient` and emits a `payment-received` action the moment an SOL
/// transfer of at least `expected_lamports` is confirmed.
///
/// Uses the standard Solana PubSub protocol, which Triton One PAYG endpoints
/// support natively. No polling — the agent is woken by the network.
pub struct TritonPaymentMonitorStrategy {
    pub recipient: String,
    pub expected_lamports: u64,
    pub config: TritonConfig,
    pub label: Option<String>,
}

impl TritonPaymentMonitorStrategy {
    pub fn new(
        recipient: String,
        amount_sol: f64,
        config: TritonConfig,
        label: Option<String>,
    ) -> Self {
        Self {
            recipient,
            expected_lamports: (amount_sol * 1_000_000_000.0) as u64,
            config,
            label,
        }
    }

    fn solana_pay_url(&self, amount_sol: f64) -> String {
        encode_transfer_url(&TransferUrlFields {
            recipient: self.recipient.clone(),
            amount: Some(amount_sol),
            spl_token: None,
            reference: None,
            label: self.label.clone().or_else(|| Some("DataFeed".to_string())),
            message: Some("Pay to receive data".to_string()),
            memo: None,
        })
    }

    async fn run_stream(&self, state: &Arc<Mutex<AgentState>>) -> anyhow::Result<()> {
        let pubkey = Pubkey::from_str(&self.recipient)?;
        let rpc = RpcClient::new(self.config.rpc_url.clone());

        // Snapshot balance before subscribing so we detect the delta correctly.
        let baseline = rpc.get_balance(&pubkey).await?;

        let pubsub = PubsubClient::new(&self.config.ws_url).await?;
        let (mut stream, _unsub) = pubsub
            .account_subscribe(
                &pubkey,
                Some(RpcAccountInfoConfig {
                    commitment: Some(CommitmentConfig::confirmed()),
                    ..Default::default()
                }),
            )
            .await?;

        {
            let mut s = state.lock().unwrap();
            s.actions.push(AgentAction {
                timestamp: Utc::now(),
                action_type: "stream-connected".to_string(),
                details: format!(
                    "Triton PubSub stream open — watching {} (baseline: {} SOL)",
                    self.recipient,
                    baseline as f64 / 1_000_000_000.0
                ),
                tx_signature: None,
                slot: None,
                latency_ms: 0,
            });
        }

        let mut last_lamports = baseline;

        while let Some(response) = stream.next().await {
            {
                let s = state.lock().unwrap();
                if !s.is_running {
                    return Ok(());
                }
            }

            let current = response.value.lamports;
            if current <= last_lamports {
                last_lamports = current;
                continue;
            }

            let received = current - last_lamports;
            last_lamports = current;

            // Best-effort: most-recent confirmed signature for this address.
            let sig_slot = rpc
                .get_signatures_for_address(&pubkey)
                .await
                .ok()
                .and_then(|v| v.into_iter().next())
                .map(|s| (s.signature, s.slot));

            let action_type = if received >= self.expected_lamports {
                "payment-received"
            } else {
                "partial-payment"
            };

            let mut s = state.lock().unwrap();
            s.actions.push(AgentAction {
                timestamp: Utc::now(),
                action_type: action_type.to_string(),
                details: format!(
                    "amount: {:.9} SOL",
                    received as f64 / 1_000_000_000.0
                ),
                tx_signature: sig_slot.as_ref().map(|(sig, _)| sig.clone()),
                slot: sig_slot.map(|(_, slot)| slot),
                latency_ms: 0,
            });
        }

        Ok(())
    }
}

#[async_trait]
impl Strategy for TritonPaymentMonitorStrategy {
    fn name(&self) -> &'static str {
        "triton-payment-monitor"
    }

    async fn run(&self, state: Arc<Mutex<AgentState>>) {
        let amount_sol = self.expected_lamports as f64 / 1_000_000_000.0;

        {
            let mut s = state.lock().unwrap();
            s.rpc_endpoint = self.config.rpc_url.clone();
            s.network = self.config.network.clone();
            s.actions.push(AgentAction {
                timestamp: Utc::now(),
                action_type: "url-generated".to_string(),
                details: self.solana_pay_url(amount_sol),
                tx_signature: None,
                slot: None,
                latency_ms: 0,
            });
        }

        let mut backoff = Duration::from_secs(1);

        loop {
            {
                let s = state.lock().unwrap();
                if !s.is_running {
                    return;
                }
            }

            match self.run_stream(&state).await {
                Ok(()) => return,
                Err(e) => {
                    {
                        let mut s = state.lock().unwrap();
                        if !s.is_running {
                            return;
                        }
                        s.actions.push(AgentAction {
                            timestamp: Utc::now(),
                            action_type: "stream-error".to_string(),
                            details: format!(
                                "stream error: {} — retrying in {}s",
                                e,
                                backoff.as_secs()
                            ),
                            tx_signature: None,
                            slot: None,
                            latency_ms: 0,
                        });
                    }
                    tokio::time::sleep(backoff).await;
                    backoff = (backoff * 2).min(Duration::from_secs(30));
                }
            }
        }
    }
}
