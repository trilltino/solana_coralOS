use async_trait::async_trait;
use chrono::Utc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::agent::AgentAction;
use crate::agent::AgentState;
use crate::strategy::Strategy;
use super::payment::demo_payment_flow;
use super::url::{encode_transfer_url, TransferUrlFields};

/// Strategy for a Solana Pay transfer agent (idle until commands are issued).
pub struct TransferStrategy;

impl Default for TransferStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl TransferStrategy {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Strategy for TransferStrategy {
    async fn run(&self, state: Arc<Mutex<AgentState>>) {
        {
            let mut s = state.lock().unwrap();
            let rpc = s.rpc_endpoint.clone();
            s.actions.push(AgentAction {
                timestamp: Utc::now(),
                action_type: "strategy-start".to_string(),
                details: format!(
                    "TransferStrategy started — waiting for commands (RPC: {})",
                    rpc
                ),
                tx_signature: None,
                slot: None,
                latency_ms: 0,
            });
        }

        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let s = state.lock().unwrap();
            if !s.is_running {
                break;
            }
        }
    }

    fn name(&self) -> &'static str {
        "solana-pay-transfer"
    }

    async fn handle_message(&self, text: &str, state: Arc<Mutex<AgentState>>) -> String {
        // Expect JSON: {"recipient":"...", "amount":0.01, "label":"..."}
        if let Ok(req) = serde_json::from_str::<serde_json::Value>(text) {
            let recipient = req["recipient"].as_str().unwrap_or("").to_string();
            let amount = req["amount"].as_f64();
            let label = req["label"].as_str().map(str::to_owned);
            let url = encode_transfer_url(&TransferUrlFields {
                recipient,
                amount,
                label,
                spl_token: None,
                reference: None,
                message: None,
                memo: None,
            });
            state.lock().unwrap().actions.push(AgentAction {
                timestamp: Utc::now(),
                action_type: "coral-url-generated".to_string(),
                details: url.clone(),
                tx_signature: None,
                slot: None,
                latency_ms: 0,
            });
            return url;
        }
        "error: expected {\"recipient\":\"...\",\"amount\":0.01}".to_string()
    }
}

/// Strategy for a Solana Pay payment agent (optionally polls sandbox endpoints).
pub struct PaymentStrategy {
    pub sandbox_endpoint: Option<String>,
}

impl Default for PaymentStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl PaymentStrategy {
    pub fn new() -> Self {
        Self {
            sandbox_endpoint: Some("https://debugger.pay.sh/mpp/quote/AAPL".to_string()),
        }
    }

    pub fn with_endpoint(endpoint: String) -> Self {
        Self {
            sandbox_endpoint: Some(endpoint),
        }
    }
}

#[async_trait]
impl Strategy for PaymentStrategy {
    async fn run(&self, state: Arc<Mutex<AgentState>>) {
        {
            let mut s = state.lock().unwrap();
            let rpc = s.rpc_endpoint.clone();
            s.actions.push(AgentAction {
                timestamp: Utc::now(),
                action_type: "strategy-start".to_string(),
                details: format!(
                    "PaymentStrategy started — monitoring for 402 endpoints (RPC: {})",
                    rpc
                ),
                tx_signature: None,
                slot: None,
                latency_ms: 0,
            });
        }

        // Optional: periodically check a demo endpoint
        if let Some(ref endpoint) = self.sandbox_endpoint {
            let mut ticker = tokio::time::interval(Duration::from_secs(30));
            // Skip first tick
            ticker.tick().await;

            loop {
                ticker.tick().await;
                let running = {
                    let s = state.lock().unwrap();
                    s.is_running
                };
                if !running {
                    break;
                }

                let result = demo_payment_flow(endpoint, 1_000_000).await;
                let mut s = state.lock().unwrap();
                s.actions.push(AgentAction {
                    timestamp: Utc::now(),
                    action_type: "x402-demo-poll".to_string(),
                    details: format!(
                        "Polled {} — success={} challenge={:?}",
                        endpoint,
                        result.success,
                        result.challenge.as_ref().map(|c| format!("{} {}", c.amount, c.token))
                    ),
                    tx_signature: None,
                    slot: None,
                    latency_ms: 0,
                });
            }
        } else {
            loop {
                tokio::time::sleep(Duration::from_secs(1)).await;
                let s = state.lock().unwrap();
                if !s.is_running {
                    break;
                }
            }
        }
    }

    fn name(&self) -> &'static str {
        "solana-pay-payment"
    }

    async fn handle_message(&self, text: &str, state: Arc<Mutex<AgentState>>) -> String {
        // Expect JSON: {"endpoint":"https://...","budget":1000000}
        if let Ok(req) = serde_json::from_str::<serde_json::Value>(text) {
            let endpoint = req["endpoint"].as_str().unwrap_or("").to_string();
            let budget = req["budget"].as_u64().unwrap_or(1_000_000);
            let result = demo_payment_flow(&endpoint, budget).await;
            let summary = serde_json::to_string(&result).unwrap_or_default();
            state.lock().unwrap().actions.push(AgentAction {
                timestamp: Utc::now(),
                action_type: "coral-payment-result".to_string(),
                details: summary.chars().take(200).collect(),
                tx_signature: None,
                slot: None,
                latency_ms: 0,
            });
            return summary;
        }
        "error: expected {\"endpoint\":\"https://...\",\"budget\":1000000}".to_string()
    }
}

/// Strategy that delivers real weather data from open-meteo.com (no API key required).
///
/// Coral mention format:
///   {"city":"London"}  — uses geocoding to resolve lat/lon first
///   {"lat":51.5,"lon":-0.1}  — direct coordinates
pub struct WeatherStrategy;

impl WeatherStrategy {
    pub fn new() -> Self { Self }
}

impl Default for WeatherStrategy {
    fn default() -> Self { Self::new() }
}

#[async_trait]
impl Strategy for WeatherStrategy {
    async fn run(&self, state: Arc<Mutex<AgentState>>) {
        state.lock().unwrap().actions.push(AgentAction {
            timestamp: Utc::now(),
            action_type: "strategy-start".to_string(),
            details: "WeatherStrategy ready — send {\"city\":\"London\"} to get live weather".to_string(),
            tx_signature: None,
            slot: None,
            latency_ms: 0,
        });
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            if !state.lock().unwrap().is_running { break; }
        }
    }

    fn name(&self) -> &'static str { "weather" }

    async fn handle_message(&self, text: &str, state: Arc<Mutex<AgentState>>) -> String {
        let start = std::time::Instant::now();
        let result = fetch_weather(text).await;
        let latency = start.elapsed().as_millis() as u64;

        let (action_type, details) = match &result {
            Ok(json) => ("data-delivered", json.chars().take(200).collect::<String>()),
            Err(e) => ("rpc-error", e.clone()),
        };

        state.lock().unwrap().actions.push(AgentAction {
            timestamp: Utc::now(),
            action_type: action_type.to_string(),
            details,
            tx_signature: None,
            slot: None,
            latency_ms: latency,
        });

        result.unwrap_or_else(|e| format!("{{\"error\":\"{}\"}}", e))
    }
}

async fn fetch_weather(text: &str) -> Result<String, String> {
    let client = reqwest::Client::new();

    // Parse request — accept {city} or {lat,lon}
    let (lat, lon, city_name) = if let Ok(req) = serde_json::from_str::<serde_json::Value>(text) {
        if let (Some(lat), Some(lon)) = (req["lat"].as_f64(), req["lon"].as_f64()) {
            (lat, lon, req["city"].as_str().unwrap_or("unknown").to_string())
        } else if let Some(city) = req["city"].as_str() {
            // Geocode via open-meteo geocoding API (no key needed)
            let geo_url = format!(
                "https://geocoding-api.open-meteo.com/v1/search?name={}&count=1&language=en&format=json",
                urlencoding::encode(city)
            );
            let geo: serde_json::Value = client.get(&geo_url).send().await
                .map_err(|e| format!("geocoding request failed: {}", e))?
                .json().await
                .map_err(|e| format!("geocoding parse failed: {}", e))?;

            let result = geo["results"].get(0)
                .ok_or_else(|| format!("city '{}' not found", city))?;

            let lat = result["latitude"].as_f64()
                .ok_or("latitude missing from geocoding result")?;
            let lon = result["longitude"].as_f64()
                .ok_or("longitude missing from geocoding result")?;
            let name = result["name"].as_str().unwrap_or(city).to_string();
            (lat, lon, name)
        } else {
            return Err("expected {\"city\":\"London\"} or {\"lat\":51.5,\"lon\":-0.1}".to_string());
        }
    } else {
        // Try treating plain text as a city name
        let city = text.trim();
        let geo_url = format!(
            "https://geocoding-api.open-meteo.com/v1/search?name={}&count=1&language=en&format=json",
            urlencoding::encode(city)
        );
        let geo: serde_json::Value = client.get(&geo_url).send().await
            .map_err(|e| format!("geocoding failed: {}", e))?
            .json().await
            .map_err(|e| format!("geocoding parse failed: {}", e))?;
        let result = geo["results"].get(0)
            .ok_or_else(|| format!("city '{}' not found", city))?;
        let lat = result["latitude"].as_f64().ok_or("latitude missing")?;
        let lon = result["longitude"].as_f64().ok_or("longitude missing")?;
        let name = result["name"].as_str().unwrap_or(city).to_string();
        (lat, lon, name)
    };

    // Fetch current weather from open-meteo (free, no key)
    let weather_url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&current=temperature_2m,relative_humidity_2m,wind_speed_10m,weather_code&wind_speed_unit=mph&timezone=auto",
        lat, lon
    );
    let weather: serde_json::Value = client.get(&weather_url).send().await
        .map_err(|e| format!("weather request failed: {}", e))?
        .json().await
        .map_err(|e| format!("weather parse failed: {}", e))?;

    let current = weather["current"].as_object()
        .ok_or("missing current weather block")?;

    let temp_c = current["temperature_2m"].as_f64().unwrap_or(0.0);
    let humidity = current["relative_humidity_2m"].as_f64().unwrap_or(0.0);
    let wind_mph = current["wind_speed_10m"].as_f64().unwrap_or(0.0);
    let code = current["weather_code"].as_u64().unwrap_or(0);

    let condition = weather_code_to_label(code);

    let response = serde_json::json!({
        "city": city_name,
        "lat": lat,
        "lon": lon,
        "temperature_c": temp_c,
        "temperature_f": (temp_c * 9.0 / 5.0 + 32.0).round() / 10.0 * 10.0,
        "humidity_pct": humidity,
        "wind_mph": wind_mph,
        "condition": condition,
        "weather_code": code,
        "source": "open-meteo.com (no API key)",
        "fetched_at": Utc::now().to_rfc3339(),
    });

    Ok(serde_json::to_string_pretty(&response).unwrap_or_default())
}

fn weather_code_to_label(code: u64) -> &'static str {
    match code {
        0 => "Clear sky",
        1 => "Mainly clear",
        2 => "Partly cloudy",
        3 => "Overcast",
        45 | 48 => "Foggy",
        51 | 53 | 55 => "Drizzle",
        61 | 63 | 65 => "Rain",
        71 | 73 | 75 => "Snow",
        80 | 81 | 82 => "Rain showers",
        95 => "Thunderstorm",
        96 | 99 => "Thunderstorm with hail",
        _ => "Unknown",
    }
}
