use serde::{Deserialize, Serialize};
use solana_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use std::str::FromStr;

/// Result of validating a Solana Pay transfer transaction.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub signature: String,
    pub recipient_found: bool,
    pub amount_transferred: Option<f64>,
    pub token_mint: Option<String>,
    pub token_symbol: Option<String>,
    pub sender: Option<String>,
    pub description: Option<String>,
    pub slot: Option<u64>,
    pub confirmations: Option<u64>,
    pub timestamp: Option<u64>,
    pub fee_lamports: Option<u64>,
    pub error: Option<String>,
}

/// Validate a transaction as a Solana Pay transfer.
/// Checks that the tx exists and the recipient is in the account list.
pub async fn validate_transfer(
    rpc_url: &str,
    signature: &str,
    expected_recipient: Option<&str>,
) -> ValidationResult {
    let client = RpcClient::new_with_commitment(
        rpc_url.to_string(),
        CommitmentConfig::confirmed(),
    );

    let tx_result = match client.get_transaction_with_config(
        &match solana_sdk::signature::Signature::from_str(signature) {
            Ok(sig) => sig,
            Err(e) => return ValidationResult {
                valid: false,
                signature: signature.to_string(),
                recipient_found: false,
                amount_transferred: None,
                token_mint: None,
                token_symbol: None,
                sender: None,
                description: None,
                slot: None,
                confirmations: None,
                timestamp: None,
                fee_lamports: None,
                error: Some(format!("Invalid signature: {}", e)),
            },
        },
        solana_client::rpc_config::RpcTransactionConfig {
            encoding: Some(solana_transaction_status::UiTransactionEncoding::JsonParsed),
            commitment: Some(CommitmentConfig::confirmed()),
            max_supported_transaction_version: Some(0),
        },
    ) {
        Ok(tx) => tx,
        Err(e) => {
            return ValidationResult {
                valid: false,
                signature: signature.to_string(),
                recipient_found: false,
                amount_transferred: None,
                token_mint: None,
                token_symbol: None,
                sender: None,
                description: None,
                slot: None,
                confirmations: None,
                timestamp: None,
                fee_lamports: None,
                error: Some(format!("RPC error: {}", e)),
            };
        }
    };

    let meta = match tx_result.transaction.meta {
        Some(m) => m,
        None => {
            return ValidationResult {
                valid: false,
                signature: signature.to_string(),
                recipient_found: false,
                amount_transferred: None,
                token_mint: None,
                token_symbol: None,
                sender: None,
                description: None,
                slot: Some(tx_result.slot),
                confirmations: None,
                timestamp: None,
                fee_lamports: None,
                error: Some("Transaction has no metadata".to_string()),
            };
        }
    };

    if meta.err.is_some() {
        return ValidationResult {
            valid: false,
            signature: signature.to_string(),
            recipient_found: false,
            amount_transferred: None,
            token_mint: None,
            token_symbol: None,
            sender: None,
            description: None,
            slot: Some(tx_result.slot),
            confirmations: None,
            timestamp: None,
            fee_lamports: Some(meta.fee),
            error: Some("Transaction failed".to_string()),
        };
    }

    let slot = tx_result.slot;
    let block_time = tx_result.block_time;

    // Extract account keys and compute transfer amounts in one pass.
    let (recipient_found, amount_transferred, sender) = {
        let encoded = tx_result.transaction.transaction;
        match encoded {
            solana_transaction_status::EncodedTransaction::Json(ui_tx) => {
                let keys: Vec<String> = match ui_tx.message {
                    solana_transaction_status::UiMessage::Parsed(parsed) => {
                        parsed.account_keys.into_iter().map(|a| a.pubkey).collect()
                    }
                    solana_transaction_status::UiMessage::Raw(raw) => raw.account_keys,
                };

                let found = expected_recipient
                    .map(|r| keys.iter().any(|k| k == r))
                    .unwrap_or(true);

                // Balance increase at recipient's account index = amount received.
                let amount = expected_recipient
                    .and_then(|r| keys.iter().position(|k| k == r))
                    .and_then(|idx| {
                        let pre = meta.pre_balances.get(idx)?;
                        let post = meta.post_balances.get(idx)?;
                        post.checked_sub(*pre)
                            .map(|diff| diff as f64 / 1_000_000_000.0)
                    });

                // First account is the fee-payer / sender.
                let from = keys.into_iter().next();

                (found, amount, from)
            }
            _ => (expected_recipient.is_none(), None, None),
        }
    };

    ValidationResult {
        valid: recipient_found,
        signature: signature.to_string(),
        recipient_found,
        amount_transferred,
        token_mint: None,
        token_symbol: None,
        sender,
        description: None,
        slot: Some(slot),
        confirmations: None,
        timestamp: block_time.map(|t| t as u64),
        fee_lamports: Some(meta.fee),
        error: None,
    }
}
