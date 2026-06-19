use serde::{Deserialize, Serialize};

/// Detected payment protocol.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum PaymentProtocol {
    #[serde(rename = "mpp")]
    Mpp,
    #[serde(rename = "x402")]
    X402,
}

impl std::fmt::Display for PaymentProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PaymentProtocol::Mpp => write!(f, "mpp"),
            PaymentProtocol::X402 => write!(f, "x402"),
        }
    }
}

/// Extracted payment challenge from a 402 response.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PaymentChallenge {
    pub protocol: PaymentProtocol,
    /// Amount in the smallest unit (lamports for SOL, raw for SPL).
    pub amount: u64,
    pub recipient: String,
    pub token: String,
    /// Raw challenge payload (base64 for MPP, JSON for x402).
    pub payload: String,
}

/// Result of a demo payment flow against a sandbox endpoint.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DemoPaymentResult {
    pub success: bool,
    pub endpoint: String,
    pub challenge: Option<PaymentChallenge>,
    pub payment_header: Option<String>,
    pub response_body: Option<String>,
    pub error: Option<String>,
}

/// Parse a 402 response and extract the payment challenge.
/// Supports both MPP (`www-authenticate`) and x402 (`x-payment-required`).
pub fn parse_402_response(headers: &[(String, String)]) -> Option<PaymentChallenge> {
    // Try MPP first
    for (name, value) in headers {
        let lower = name.to_ascii_lowercase();
        if lower == "www-authenticate" {
            if let Some(challenge) = parse_mpp_challenge(value) {
                return Some(challenge);
            }
        }
    }

    // Try x402
    for (name, value) in headers {
        let lower = name.to_ascii_lowercase();
        if lower == "x-payment-required" || lower == "x-payment" {
            if let Some(challenge) = parse_x402_challenge(value) {
                return Some(challenge);
            }
        }
    }

    None
}

fn parse_mpp_challenge(header_value: &str) -> Option<PaymentChallenge> {
    // MPP format: "Solana mpp=<base64-challenge>"
    let trimmed = header_value.trim();
    if !trimmed.to_ascii_lowercase().starts_with("solana") {
        return None;
    }

    // Extract the mpp=... portion
    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    for part in parts {
        if part.to_ascii_lowercase().starts_with("mpp=") {
            let payload = &part[4..]; // after "mpp="
            // Try to decode base64 and extract amount/recipient
            if let Ok(decoded) = base64::Engine::decode(
                &base64::engine::general_purpose::STANDARD,
                payload,
            ) {
                if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&decoded) {
                    let request = json.get("request")?;
                    let amount = request
                        .get("amount")?
                        .as_u64()
                        .or_else(|| request.get("amount")?.as_str()?.parse().ok())?;
                    let currency = request
                        .get("currency")?
                        .as_str()
                        .unwrap_or("USDC")
                        .to_string();
                    let method_details = request.get("method_details")?;
                    let recipient = method_details
                        .get("destination")?
                        .as_str()?
                        .to_string();

                    return Some(PaymentChallenge {
                        protocol: PaymentProtocol::Mpp,
                        amount,
                        recipient,
                        token: currency,
                        payload: payload.to_string(),
                    });
                }
            }

            // Fallback: return raw challenge without decoding
            return Some(PaymentChallenge {
                protocol: PaymentProtocol::Mpp,
                amount: 0,
                recipient: String::new(),
                token: "USDC".to_string(),
                payload: payload.to_string(),
            });
        }
    }

    None
}

fn parse_x402_challenge(header_value: &str) -> Option<PaymentChallenge> {
    let json: serde_json::Value = serde_json::from_str(header_value).ok()?;

    let scheme = json.get("scheme")?.as_str()?;
    if scheme != "solana" {
        return None;
    }

    let requirements = json.get("requirements")?;
    let amount = requirements
        .get("amount")?
        .as_str()?
        .parse::<u64>()
        .ok()?;
    let currency = requirements
        .get("currency")?
        .as_str()
        .unwrap_or("USDC")
        .to_string();
    let recipient = requirements
        .get("recipient")?
        .as_str()?
        .to_string();

    Some(PaymentChallenge {
        protocol: PaymentProtocol::X402,
        amount,
        recipient,
        token: currency,
        payload: header_value.to_string(),
    })
}

/// Run the full demo payment flow against a sandbox endpoint.
/// Returns the payment result without real signing (demo mode).
pub async fn demo_payment_flow(endpoint: &str, _budget: u64) -> DemoPaymentResult {
    let client = reqwest::Client::new();

    // Step 1: initial request
    let initial = match client.get(endpoint).send().await {
        Ok(r) => r,
        Err(e) => {
            return DemoPaymentResult {
                success: false,
                endpoint: endpoint.to_string(),
                challenge: None,
                payment_header: None,
                response_body: None,
                error: Some(format!("Initial request failed: {}", e)),
            };
        }
    };

    if initial.status() != reqwest::StatusCode::PAYMENT_REQUIRED {
        let body = initial.text().await.ok();
        return DemoPaymentResult {
            success: true,
            endpoint: endpoint.to_string(),
            challenge: None,
            payment_header: None,
            response_body: body,
            error: None,
        };
    }

    // Step 2: extract headers
    let mut headers_vec: Vec<(String, String)> = Vec::new();
    for (key, value) in initial.headers() {
        if let Ok(v) = value.to_str() {
            headers_vec.push((key.to_string(), v.to_string()));
        }
    }

    let challenge = match parse_402_response(&headers_vec) {
        Some(c) => c,
        None => {
            return DemoPaymentResult {
                success: false,
                endpoint: endpoint.to_string(),
                challenge: None,
                payment_header: None,
                response_body: None,
                error: Some("Could not parse 402 challenge".to_string()),
            };
        }
    };

    // Step 3: build mock payment header (demo mode — no real signing)
    let payment_header = format!(
        "Bearer demo-payment-{}-to-{}-amount-{}",
        challenge.protocol.to_string().to_lowercase(),
        challenge.recipient,
        challenge.amount
    );

    // Step 4: retry with payment
    let retry = match client
        .get(endpoint)
        .header("Authorization", &payment_header)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return DemoPaymentResult {
                success: false,
                endpoint: endpoint.to_string(),
                challenge: Some(challenge),
                payment_header: Some(payment_header),
                response_body: None,
                error: Some(format!("Retry failed: {}", e)),
            };
        }
    };

    let status = retry.status();
    let body = retry.text().await.ok();

    DemoPaymentResult {
        success: status.is_success(),
        endpoint: endpoint.to_string(),
        challenge: Some(challenge),
        payment_header: Some(payment_header),
        response_body: body,
        error: if status.is_success() {
            None
        } else {
            Some(format!("Retry returned {}", status))
        },
    }
}
