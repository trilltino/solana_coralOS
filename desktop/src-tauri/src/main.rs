// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use chrono::Utc;
use agent_core::{
    AgentAction, AgentManager, AgentMessage, AgentMeta, AgentRole, AgentState, PayMode,
    SharedStateEntry, StateChange, Workflow, WorkflowStep,
    solana_pay::{DemoPaymentResult, ParsedUrl, PaymentChallenge, ValidationResult},
};
use serde_json::Value;
use tauri::State;

mod coralos;
use coralos::{CoralOSClient, SessionStateExtended};

mod python_agent;
use python_agent::PythonAgentState;

/// A recorded payment flow capturing the full request → 402 → payment → delivery lifecycle.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct PaymentFlowRecord {
    id: String,
    agent_id: String,
    endpoint: String,
    status: String,
    protocol: Option<String>,
    amount: Option<u64>,
    recipient: Option<String>,
    token: Option<String>,
    payment_header: Option<String>,
    response_body: Option<String>,
    error: Option<String>,
    request_at: String,
    challenge_at: Option<String>,
    payment_at: Option<String>,
    delivery_at: Option<String>,
}

struct AppState {
    manager: AgentManager,
    coralos: CoralOSClient,
    flows: std::sync::Mutex<Vec<PaymentFlowRecord>>,
}

fn store_flow(flows: &std::sync::Mutex<Vec<PaymentFlowRecord>>, record: PaymentFlowRecord) {
    if let Ok(mut v) = flows.lock() {
        v.push(record);
        // Keep at most 100 flows
        if v.len() > 100 {
            let excess = v.len() - 100;
            v.drain(0..excess);
        }
    }
}

// --- Multi-agent commands ---

#[tauri::command]
fn create_agent(state: State<AppState>, id: String) -> Result<AgentState, String> {
    state.manager
        .create_agent(id)
        .ok_or_else(|| "Agent with this ID already exists".to_string())
}

#[tauri::command]
fn list_agents(state: State<AppState>) -> Result<Vec<(String, AgentState)>, String> {
    Ok(state.manager.list_agents())
}

#[tauri::command]
fn delete_agent(state: State<AppState>, id: String) -> Result<bool, String> {
    Ok(state.manager.remove_agent(&id))
}

#[tauri::command]
fn get_agent_state(state: State<AppState>, id: String) -> Result<AgentState, String> {
    state.manager
        .get_agent_state(&id)
        .ok_or_else(|| "Agent not found".to_string())
}

#[tauri::command]
fn set_agent_rpc(state: State<AppState>, id: String, url: String) -> Result<bool, String> {
    Ok(state.manager.set_rpc(&id, url))
}

#[tauri::command]
fn set_agent_triton(
    state: State<AppState>,
    id: String,
    x_token: String,
    grpc_endpoint: Option<String>,
) -> Result<bool, String> {
    use agent_core::triton::TritonConfig;
    let config = match grpc_endpoint {
        Some(ep) => TritonConfig::custom(ep.clone(), "https://api.mainnet-beta.solana.com", ep.replace("https://", "wss://").replace("http://", "ws://"), x_token, "mainnet-beta"),
        None => TritonConfig::mainnet(x_token),
    };
    Ok(state.manager.set_triton(&id, &config))
}

#[tauri::command]
async fn start_agent(state: State<'_, AppState>, id: String) -> Result<bool, String> {
    state.manager
        .start_agent(&id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn stop_agent(state: State<AppState>, id: String) -> Result<bool, String> {
    Ok(state.manager.stop_agent(&id))
}

#[tauri::command]
fn get_agent_actions(state: State<AppState>, id: String) -> Result<Vec<AgentAction>, String> {
    state.manager
        .get_actions(&id)
        .ok_or_else(|| "Agent not found".to_string())
}

// --- Agent Role commands ---

#[tauri::command]
fn set_agent_role(state: State<AppState>, id: String, role: String) -> Result<bool, String> {
    let role_enum = match role.as_str() {
        "leader" => AgentRole::Leader,
        "worker" => AgentRole::Worker,
        "monitor" => AgentRole::Monitor,
        "analyst" => AgentRole::Analyst,
        "trader" => AgentRole::Trader,
        "coordinator" => AgentRole::Coordinator,
        _ => return Err("Invalid role".to_string()),
    };
    Ok(state.manager.set_agent_role(&id, role_enum))
}

#[tauri::command]
fn get_agent_meta(state: State<AppState>, id: String) -> Result<AgentMeta, String> {
    state.manager
        .get_agent_meta(&id)
        .ok_or_else(|| "Agent not found".to_string())
}

#[tauri::command]
fn list_agents_with_roles(
    state: State<AppState>,
) -> Result<Vec<(String, AgentState, AgentMeta)>, String> {
    Ok(state.manager.list_agents_with_roles())
}

// --- Messaging commands ---

#[tauri::command]
fn send_message(
    state: State<AppState>,
    from: String,
    to: Option<String>,
    msg_type: String,
    payload: String,
) -> Result<bool, String> {
    let msg = if let Some(to_id) = to {
        AgentMessage::direct(from, to_id, msg_type, payload)
    } else {
        AgentMessage::broadcast(from, msg_type, payload)
    };
    state.manager.send_message(msg);
    Ok(true)
}

#[tauri::command]
fn get_messages(state: State<AppState>, agent_id: String) -> Result<Vec<AgentMessage>, String> {
    Ok(state.manager.get_messages(&agent_id))
}

#[tauri::command]
fn get_all_messages(state: State<AppState>) -> Result<Vec<AgentMessage>, String> {
    Ok(state.manager.get_all_messages())
}

#[tauri::command]
fn get_conversation(
    state: State<AppState>,
    agent_a: String,
    agent_b: String,
) -> Result<Vec<AgentMessage>, String> {
    Ok(state.manager.get_conversation(&agent_a, &agent_b))
}

// --- Shared State commands ---

#[tauri::command]
fn set_shared_state(
    state: State<AppState>,
    key: String,
    value: Value,
    changed_by: String,
) -> Result<bool, String> {
    Ok(state.manager.set_shared_state(&key, value, &changed_by))
}

#[tauri::command]
fn get_shared_state(
    state: State<AppState>,
    key: String,
) -> Result<Option<SharedStateEntry>, String> {
    Ok(state.manager.get_shared_state(&key))
}

#[tauri::command]
fn get_all_shared_state(
    state: State<AppState>,
) -> Result<std::collections::HashMap<String, SharedStateEntry>, String> {
    Ok(state.manager.get_all_shared_state())
}

#[tauri::command]
fn delete_shared_state(
    state: State<AppState>,
    key: String,
    changed_by: String,
) -> Result<bool, String> {
    Ok(state.manager.delete_shared_state(&key, &changed_by))
}

#[tauri::command]
fn get_state_history(state: State<AppState>) -> Result<Vec<StateChange>, String> {
    Ok(state.manager.get_state_history())
}

// --- Workflow commands ---

#[tauri::command]
fn create_workflow(
    state: State<AppState>,
    id: String,
    name: String,
    description: String,
    steps: Vec<WorkflowStep>,
    priority: u8,
    created_by: String,
) -> Result<bool, String> {
    let mut workflow = Workflow::new(&id, &name, &description, &created_by);
    workflow.priority = priority.clamp(1, 10);
    for step in steps {
        workflow.add_step(step);
    }
    state.manager.create_workflow(workflow);
    Ok(true)
}

#[tauri::command]
fn get_workflow(state: State<AppState>, id: String) -> Result<Option<Workflow>, String> {
    Ok(state.manager.get_workflow(&id))
}

#[tauri::command]
fn list_workflows(state: State<AppState>) -> Result<Vec<Workflow>, String> {
    Ok(state.manager.list_workflows())
}

#[tauri::command]
fn delete_workflow(state: State<AppState>, id: String) -> Result<bool, String> {
    Ok(state.manager.delete_workflow(&id))
}

#[tauri::command]
fn assign_workflow_step(
    state: State<AppState>,
    workflow_id: String,
    step_id: String,
    agent_id: String,
) -> Result<bool, String> {
    Ok(state.manager.assign_workflow_step(&workflow_id, &step_id, &agent_id))
}

#[tauri::command]
fn start_workflow_step(
    state: State<AppState>,
    workflow_id: String,
    step_id: String,
) -> Result<bool, String> {
    Ok(state.manager.start_workflow_step(&workflow_id, &step_id))
}

#[tauri::command]
fn complete_workflow_step(
    state: State<AppState>,
    workflow_id: String,
    step_id: String,
    result: String,
) -> Result<bool, String> {
    Ok(state.manager.complete_workflow_step(&workflow_id, &step_id, result))
}

#[tauri::command]
fn fail_workflow_step(
    state: State<AppState>,
    workflow_id: String,
    step_id: String,
    reason: String,
) -> Result<bool, String> {
    Ok(state.manager.fail_workflow_step(&workflow_id, &step_id, reason))
}

#[tauri::command]
fn get_agent_workflows(state: State<AppState>, agent_id: String) -> Result<Vec<Workflow>, String> {
    Ok(state.manager.get_agent_workflows(&agent_id))
}

#[tauri::command]
fn get_active_workflows(state: State<AppState>) -> Result<Vec<Workflow>, String> {
    Ok(state.manager.get_active_workflows())
}

// --- Helius agent commands ---

#[tauri::command]
fn set_agent_helius(state: State<AppState>, id: String, api_key: String) -> Result<bool, String> {
    let rpc = if api_key.trim().is_empty() {
        "https://api.devnet.solana.com".to_string()
    } else {
        format!("https://devnet.helius-rpc.com/?api-key={}", api_key.trim())
    };
    Ok(state.manager.set_rpc(&id, rpc))
}

#[tauri::command]
fn create_helius_monitor_agent(
    state: State<AppState>,
    id: String,
    wallet: String,
    amount_sol: f64,
    api_key: String,
    label: Option<String>,
) -> Result<AgentState, String> {
    use agent_core::triton::TritonConfig;
    let (rpc, ws) = if api_key.trim().is_empty() {
        (
            "https://api.devnet.solana.com".to_string(),
            "wss://api.devnet.solana.com".to_string(),
        )
    } else {
        let key = api_key.trim();
        (
            format!("https://devnet.helius-rpc.com/?api-key={}", key),
            format!("wss://devnet.helius-rpc.com/?api-key={}", key),
        )
    };
    let config = TritonConfig::custom(rpc.clone(), rpc.clone(), ws, api_key, "devnet");
    state.manager
        .create_triton_monitor_agent(id, wallet, amount_sol, config, label)
        .ok_or_else(|| "Agent with this ID already exists".to_string())
}

// --- CoralOS proxy commands ---

#[tauri::command]
fn coralos_set_url(state: State<AppState>, url: String) -> Result<bool, String> {
    state.coralos.set_url(url);
    Ok(true)
}

#[tauri::command]
fn coralos_set_token(state: State<AppState>, token: String) -> Result<bool, String> {
    state.coralos.set_token(token);
    Ok(true)
}

#[tauri::command]
async fn coralos_list_sessions(
    state: State<'_, AppState>,
    namespace: String,
) -> Result<Vec<SessionStateExtended>, String> {
    state.coralos
        .list_sessions(&namespace)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn coralos_get_session(
    state: State<'_, AppState>,
    namespace: String,
    session_id: String,
) -> Result<SessionStateExtended, String> {
    state.coralos
        .get_session(&namespace, &session_id)
        .await
        .map_err(|e| e.to_string())
}

// --- CoralOS MCP commands ---

/// Join a CoralOS session as a full MCP participant.
///
/// Spawns a background task that loops: `wait_for_mention` → handler → `send_message`.
/// The handler records each mention as an agent action and echoes back a receipt.
#[tauri::command]
async fn coralos_mcp_join(
    state: State<'_, AppState>,
    connection_url: String,
    agent_id: String,
) -> Result<bool, String> {
    use agent_core::CoralMcpSession;

    let session = CoralMcpSession::connect(&connection_url, &agent_id)
        .await
        .map_err(|e| e.to_string())?;

    // Clone the manager (cheap — just clones inner Arcs) for the background task.
    let manager = state.manager.clone();
    let aid = agent_id.clone();

    tokio::spawn(async move {
        session
            .run_loop(move |mention| {
                let manager = manager.clone();
                let aid = aid.clone();
                async move {
                    let response = format!(
                        "agent={} received mention from {:?} thread={:?}",
                        aid, mention.sender, mention.thread_id
                    );
                    manager.record_action(
                        &aid,
                        agent_core::AgentAction {
                            timestamp: chrono::Utc::now(),
                            action_type: "coral-mention".to_string(),
                            details: mention.text.chars().take(200).collect(),
                            tx_signature: None,
                            slot: None,
                            latency_ms: 0,
                        },
                    );
                    response
                }
            })
            .await;
    });

    Ok(true)
}

/// Return true if the named agent exists (used as a simple MCP session proxy check).
#[tauri::command]
fn coralos_mcp_status(state: State<AppState>, agent_id: String) -> Result<bool, String> {
    Ok(state.manager.get_agent_state(&agent_id).is_some())
}

// --- Solana Pay Agent commands ---

#[tauri::command]
fn create_solana_pay_agent(
    state: State<AppState>,
    id: String,
    mode: String,
) -> Result<AgentState, String> {
    let mode_enum = match mode.as_str() {
        "transfer" => PayMode::Transfer,
        "payment" => PayMode::Payment,
        _ => return Err("Invalid mode: use 'transfer' or 'payment'".to_string()),
    };
    state.manager
        .create_solana_pay_agent(id, mode_enum)
        .ok_or_else(|| "Agent with this ID already exists".to_string())
}

#[tauri::command]
fn get_agent_capabilities(state: State<AppState>, id: String) -> Result<Vec<String>, String> {
    Ok(state.manager.get_agent_capabilities(&id))
}

#[tauri::command]
fn solana_pay_create_url(
    recipient: String,
    amount: u64,
    label: String,
    message: String,
) -> Result<String, String> {
    use agent_core::solana_pay::{TransferUrlFields, encode_transfer_url};
    let fields = TransferUrlFields {
        recipient,
        amount: Some(amount as f64 / 1_000_000_000.0),
        spl_token: None,
        reference: None,
        label: Some(label),
        message: Some(message),
        memo: None,
    };
    Ok(encode_transfer_url(&fields))
}

#[tauri::command]
fn solana_pay_parse_url(url: String) -> Result<ParsedUrl, String> {
    use agent_core::solana_pay::parse_url;
    parse_url(&url).map_err(|e| e.to_string())
}

#[tauri::command]
async fn solana_pay_validate(
    state: State<'_, AppState>,
    id: String,
    signature: String,
    expected_recipient: Option<String>,
) -> Result<ValidationResult, String> {
    let agent_state = state.manager
        .get_agent_state(&id)
        .ok_or_else(|| "Agent not found".to_string())?;

    let rpc_url = agent_state.rpc_endpoint;
    use agent_core::solana_pay::validate_transfer;
    Ok(validate_transfer(&rpc_url, &signature, expected_recipient.as_deref()).await)
}

#[tauri::command]
fn x402_parse_challenge(headers: Vec<(String, String)>) -> Result<Option<PaymentChallenge>, String> {
    use agent_core::solana_pay::parse_402_response;
    Ok(parse_402_response(&headers))
}

#[tauri::command]
async fn x402_demo_payment(
    state: State<'_, AppState>,
    endpoint: String,
    budget: u64,
) -> Result<DemoPaymentResult, String> {
    use agent_core::solana_pay::demo_payment_flow;
    let now = Utc::now().to_rfc3339();
    let result = demo_payment_flow(&endpoint, budget).await;
    let challenge_at = result.challenge.as_ref().map(|_| Utc::now().to_rfc3339());
    let payment_at = result.payment_header.as_ref().map(|_| Utc::now().to_rfc3339());
    let delivery_at = if result.success { Some(Utc::now().to_rfc3339()) } else { None };
    store_flow(&state.flows, PaymentFlowRecord {
        id: format!("demo-{}", Utc::now().timestamp_millis()),
        agent_id: "solana-pay-tab".to_string(),
        endpoint: endpoint.clone(),
        status: if result.success { "success".to_string() } else { "failed".to_string() },
        protocol: result.challenge.as_ref().map(|c| c.protocol.to_string()),
        amount: result.challenge.as_ref().map(|c| c.amount),
        recipient: result.challenge.as_ref().map(|c| c.recipient.clone()),
        token: result.challenge.as_ref().map(|c| c.token.clone()),
        payment_header: result.payment_header.clone(),
        response_body: result.response_body.clone(),
        error: result.error.clone(),
        request_at: now,
        challenge_at,
        payment_at,
        delivery_at,
    });
    Ok(result)
}

// --- Weather agent command ---

/// Query live weather for any city using WeatherStrategy (open-meteo.com, no API key).
///
/// Creates a transient AgentState, runs WeatherStrategy.handle_message, and
/// records the result as an action on the "weather-agent" if it exists in the manager.
#[tauri::command]
async fn weather_query(
    state: State<'_, AppState>,
    city: String,
) -> Result<serde_json::Value, String> {
    use agent_core::{solana_pay::WeatherStrategy, strategy::Strategy};
    use std::sync::{Arc, Mutex};

    let agent_state = Arc::new(Mutex::new(AgentState {
        is_running: true,
        actions: vec![],
        rpc_endpoint: String::new(),
        network: "devnet".to_string(),
        strategy: "weather".to_string(),
    }));

    let strategy = WeatherStrategy::new();
    let text = serde_json::json!({ "city": city }).to_string();
    let start = std::time::Instant::now();
    let result = strategy.handle_message(&text, Arc::clone(&agent_state)).await;
    let latency_ms = start.elapsed().as_millis() as u64;

    state.manager.record_action(
        "weather-agent",
        AgentAction {
            timestamp: Utc::now(),
            action_type: "data-delivered".to_string(),
            details: format!("weather for {} ({} ms)", city, latency_ms),
            tx_signature: None,
            slot: None,
            latency_ms,
        },
    );

    serde_json::from_str(&result).map_err(|_| result)
}

// --- Pay Demo commands ---

#[tauri::command]
fn create_triton_monitor_agent(
    state: State<AppState>,
    id: String,
    recipient: String,
    amount_sol: f64,
    x_token: String,
    grpc_endpoint: Option<String>,
    label: Option<String>,
) -> Result<AgentState, String> {
    use agent_core::triton::TritonConfig;
    let config = match grpc_endpoint {
        Some(ep) => TritonConfig::custom(ep.clone(), "https://api.mainnet-beta.solana.com", ep.replace("https://", "wss://").replace("http://", "ws://"), x_token, "mainnet-beta"),
        None => TritonConfig::mainnet(x_token),
    };
    state.manager
        .create_triton_monitor_agent(id, recipient, amount_sol, config, label)
        .ok_or_else(|| "Agent with this ID already exists".to_string())
}

#[tauri::command]
fn generate_solana_pay_url(
    recipient: String,
    amount_sol: f64,
    label: Option<String>,
    message: Option<String>,
) -> Result<String, String> {
    use agent_core::solana_pay::{TransferUrlFields, encode_transfer_url};
    Ok(encode_transfer_url(&TransferUrlFields {
        recipient,
        amount: Some(amount_sol),
        spl_token: None,
        reference: None,
        label,
        message,
        memo: None,
    }))
}

#[tauri::command]
fn get_pending_payment(state: State<AppState>, seller_id: String) -> Result<Option<String>, String> {
    let actions = state.manager
        .get_actions(&seller_id)
        .ok_or_else(|| "Agent not found".to_string())?;
    let sig = actions
        .iter()
        .find(|a| a.action_type == "payment-received")
        .and_then(|a| a.tx_signature.clone());
    Ok(sig)
}

/// Complete a sale: calls pay.sh to fetch real data, then delivers it agent-to-agent.
/// Returns the actual data payload received from pay.sh.
#[tauri::command]
async fn complete_sale(
    state: State<'_, AppState>,
    seller_id: String,
    buyer_id: String,
    tx_signature: Option<String>,
) -> Result<String, String> {
    use agent_core::solana_pay::demo_payment_flow;

    // Record that seller is attempting delivery
    state.manager.record_action(&seller_id, AgentAction {
        timestamp: Utc::now(),
        action_type: "fetching-data".to_string(),
        details: format!("calling pay.sh with tx={}", tx_signature.as_deref().unwrap_or("demo")),
        tx_signature: tx_signature.clone(),
        slot: None,
        latency_ms: 0,
    });

    // Call pay.sh: GET → 402 → parse challenge → pay → get real data
    let flow_request_at = Utc::now().to_rfc3339();
    let result = demo_payment_flow("https://debugger.pay.sh/mpp/quote/AAPL", 1_000_000).await;

    // Extract fields before any partial moves
    let flow_challenge_at = result.challenge.as_ref().map(|_| Utc::now().to_rfc3339());
    let flow_payment_at = result.payment_header.as_ref().map(|_| Utc::now().to_rfc3339());
    let flow_protocol = result.challenge.as_ref().map(|c| c.protocol.to_string());
    let flow_amount = result.challenge.as_ref().map(|c| c.amount);
    let flow_recipient = result.challenge.as_ref().map(|c| c.recipient.clone());
    let flow_token = result.challenge.as_ref().map(|c| c.token.clone());
    let flow_payment_header = result.payment_header.clone();
    let flow_response_body = result.response_body.clone();
    let flow_error = result.error.clone();
    let flow_success = result.success;
    let protocol = result.challenge.as_ref().map(|c| c.protocol.to_string()).unwrap_or_default();

    let data_payload = if result.success {
        result.response_body.unwrap_or_else(|| r#"{"error":"empty response"}"#.to_string())
    } else {
        // Fallback if pay.sh is unreachable
        serde_json::json!({
            "AAPL": 189.42,
            "source": "fallback",
            "error": result.error
        }).to_string()
    };

    state.manager.record_action(&seller_id, AgentAction {
        timestamp: Utc::now(),
        action_type: "data-delivered".to_string(),
        details: format!("pay.sh {} → delivered to {}", protocol, buyer_id),
        tx_signature: tx_signature.clone(),
        slot: None,
        latency_ms: 0,
    });

    state.manager.send_direct(&seller_id, &buyer_id, "data-delivered", &data_payload);

    state.manager.record_action(&buyer_id, AgentAction {
        timestamp: Utc::now(),
        action_type: "data-received".to_string(),
        details: format!("received from {} via pay.sh", seller_id),
        tx_signature: tx_signature,
        slot: None,
        latency_ms: 0,
    });

    let delivery_at = if flow_success { Some(Utc::now().to_rfc3339()) } else { None };
    store_flow(&state.flows, PaymentFlowRecord {
        id: format!("sale-{}", Utc::now().timestamp_millis()),
        agent_id: seller_id.clone(),
        endpoint: "https://debugger.pay.sh/mpp/quote/AAPL".to_string(),
        status: if flow_success { "success".to_string() } else { "failed".to_string() },
        protocol: flow_protocol,
        amount: flow_amount,
        recipient: flow_recipient,
        token: flow_token,
        payment_header: flow_payment_header,
        response_body: flow_response_body,
        error: flow_error,
        request_at: flow_request_at,
        challenge_at: flow_challenge_at,
        payment_at: flow_payment_at,
        delivery_at,
    });

    let json_val = serde_json::from_str(&data_payload)
        .unwrap_or(serde_json::Value::String(data_payload.clone()));
    state.manager.set_shared_state(
        &format!("sale/{}/result", seller_id),
        json_val,
        &seller_id,
    );

    Ok(data_payload)
}

#[tauri::command]
fn get_payment_flows(state: State<AppState>) -> Result<Vec<PaymentFlowRecord>, String> {
    state.flows
        .lock()
        .map(|v| v.clone())
        .map_err(|e| e.to_string())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AppState {
            manager: AgentManager::new(),
            coralos: CoralOSClient::new(
                "http://localhost:8080".to_string(),
                "".to_string(),
            ),
            flows: std::sync::Mutex::new(Vec::new()),
        })
        .manage(PythonAgentState::default())
        .invoke_handler(tauri::generate_handler![
            create_agent,
            list_agents,
            delete_agent,
            get_agent_state,
            set_agent_rpc,
            set_agent_triton,
            start_agent,
            stop_agent,
            get_agent_actions,
            set_agent_role,
            get_agent_meta,
            list_agents_with_roles,
            send_message,
            get_messages,
            get_all_messages,
            get_conversation,
            set_shared_state,
            get_shared_state,
            get_all_shared_state,
            delete_shared_state,
            get_state_history,
            create_workflow,
            get_workflow,
            list_workflows,
            delete_workflow,
            assign_workflow_step,
            start_workflow_step,
            complete_workflow_step,
            fail_workflow_step,
            get_agent_workflows,
            get_active_workflows,
            set_agent_helius,
            create_helius_monitor_agent,
            coralos_set_url,
            coralos_set_token,
            coralos_list_sessions,
            coralos_get_session,
            coralos_mcp_join,
            coralos_mcp_status,
            python_agent::python_agent_start,
            python_agent::python_agent_stop,
            python_agent::python_agent_status,
            create_solana_pay_agent,
            get_agent_capabilities,
            solana_pay_create_url,
            solana_pay_parse_url,
            solana_pay_validate,
            x402_parse_challenge,
            x402_demo_payment,
            weather_query,
            create_triton_monitor_agent,
            generate_solana_pay_url,
            get_pending_payment,
            complete_sale,
            get_payment_flows,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
