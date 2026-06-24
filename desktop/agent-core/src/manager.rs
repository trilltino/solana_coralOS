use crate::agent_meta::{AgentMeta, PayMode};
use crate::message_bus::{AgentMessage, MessageBus};
use crate::orchestrator::{Workflow, WorkflowEngine};
use crate::role::AgentRole;
use crate::shared_state::SharedState;
use crate::triton::TritonConfig;
use crate::solana_pay::{
    TritonPaymentMonitorStrategy, PaymentStrategy, TransferStrategy, TransferUrlFields,
    encode_transfer_url,
};
use crate::strategy::Strategy;
use crate::{Agent, AgentAction, AgentState};
use chrono::Utc;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};

/// Central coordinator: creates and drives agents, messaging, shared state, and workflows.
///
/// Internally the agent map and metadata are each protected by their own `Mutex`.
/// Callers should avoid holding one lock while acquiring the other to prevent deadlocks;
/// all methods in this file are written to release the first lock before acquiring the second.
///
/// `Clone` is cheap — it clones the inner `Arc` handles, so all clones share the same
/// underlying state.  This is used by the CoralOS MCP background task in `src-tauri`.
#[derive(Clone)]
pub struct AgentManager {
    agents: Arc<Mutex<BTreeMap<String, Arc<Agent>>>>,
    agent_meta: Arc<Mutex<HashMap<String, AgentMeta>>>,
    message_bus: MessageBus,
    shared_state: SharedState,
    workflow_engine: WorkflowEngine,
}

impl AgentManager {
    /// Create an empty manager with no agents.
    pub fn new() -> Self {
        Self {
            agents: Arc::new(Mutex::new(BTreeMap::new())),
            agent_meta: Arc::new(Mutex::new(HashMap::new())),
            message_bus: MessageBus::new(),
            shared_state: SharedState::new(),
            workflow_engine: WorkflowEngine::new(),
        }
    }

    // -------------------------------------------------------------------------
    // Agent CRUD
    // -------------------------------------------------------------------------

    /// Create a generic agent with the default RPC-poll strategy.
    ///
    /// Returns `None` if an agent with the same `id` already exists.
    pub fn create_agent(&self, id: String) -> Option<AgentState> {
        let mut agents = self.agents.lock().expect("agent map lock poisoned");
        if agents.contains_key(&id) {
            return None;
        }
        let agent = Arc::new(Agent::new());
        let state = agent.state();
        agents.insert(id.clone(), agent);
        drop(agents);

        self.agent_meta
            .lock()
            .expect("meta lock poisoned")
            .insert(id, AgentMeta::default());

        Some(state)
    }

    /// Create a Solana Pay agent configured for `mode`.
    ///
    /// - [`PayMode::Transfer`] — idle strategy that encodes transfer URLs.
    /// - [`PayMode::Payment`] — strategy that handles x402 / MPP payment challenges.
    ///
    /// Returns `None` if `id` is already taken.
    pub fn create_solana_pay_agent(&self, id: String, mode: PayMode) -> Option<AgentState> {
        let mut agents = self.agents.lock().expect("agent map lock poisoned");
        if agents.contains_key(&id) {
            return None;
        }
        let strategy: Arc<dyn Strategy> = match mode {
            PayMode::Transfer => Arc::new(TransferStrategy::new()),
            PayMode::Payment => Arc::new(PaymentStrategy::new()),
        };
        let agent = Arc::new(Agent::with_strategy(strategy));
        let state = agent.state();
        agents.insert(id.clone(), agent);
        drop(agents);

        self.agent_meta
            .lock()
            .expect("meta lock poisoned")
            .insert(id, AgentMeta { role: AgentRole::Trader, ..Default::default() });

        Some(state)
    }

    /// Create a Triton Yellowstone payment-monitor agent.
    ///
    /// The agent opens a persistent gRPC stream to Triton and emits a
    /// `payment-received` action the moment an SOL transfer of `amount_sol`
    /// lands at `recipient`. An initial `url-generated` action records the
    /// Solana Pay URI immediately.
    ///
    /// Returns `None` if `id` is already taken.
    pub fn create_triton_monitor_agent(
        &self,
        id: String,
        recipient: String,
        amount_sol: f64,
        config: TritonConfig,
        label: Option<String>,
    ) -> Option<AgentState> {
        let mut agents = self.agents.lock().expect("agent map lock poisoned");
        if agents.contains_key(&id) {
            return None;
        }

        let solana_pay_url = encode_transfer_url(&TransferUrlFields {
            recipient: recipient.clone(),
            amount: Some(amount_sol),
            spl_token: None,
            reference: None,
            label: label.clone().or_else(|| Some("DataFeed".to_string())),
            message: Some("Pay to receive data".to_string()),
            memo: None,
        });

        let strategy = Arc::new(TritonPaymentMonitorStrategy::new(
            recipient.clone(),
            amount_sol,
            config.clone(),
            label,
        ));

        let agent = Arc::new(Agent::with_strategy(strategy));
        agent.set_triton(&config);
        agent.record_action(AgentAction {
            timestamp: Utc::now(),
            action_type: "url-generated".to_string(),
            details: solana_pay_url,
            tx_signature: None,
            slot: None,
            latency_ms: 0,
        });

        let state = agent.state();
        agents.insert(id.clone(), agent);
        drop(agents);

        self.agent_meta
            .lock()
            .expect("meta lock poisoned")
            .insert(id, AgentMeta { role: AgentRole::Monitor, ..Default::default() });

        Some(state)
    }

    /// Hot-swap the strategy of a running agent.
    ///
    /// Returns `false` if no agent with `id` exists.
    pub fn set_strategy(&self, id: &str, strategy: Arc<dyn Strategy>) -> bool {
        let agents = self.agents.lock().expect("agent map lock poisoned");
        if let Some(agent) = agents.get(id) {
            agent.set_strategy(strategy);
            true
        } else {
            false
        }
    }

    /// List the capabilities exposed by an agent's current strategy.
    pub fn get_agent_capabilities(&self, id: &str) -> Vec<String> {
        let strategy_name = {
            let agents = self.agents.lock().expect("agent map lock poisoned");
            agents.get(id).map(|a| a.state().strategy)
        };
        match strategy_name.as_deref() {
            Some("solana-pay-transfer") => crate::solana_pay::get_transfer_capabilities(),
            Some("solana-pay-payment") => crate::solana_pay::get_payment_capabilities(),
            Some(_) => vec!["rpc-poll".to_string()],
            None => vec![],
        }
    }

    /// Return a live Arc reference to the agent, or `None` if not found.
    pub fn get_agent(&self, id: &str) -> Option<Arc<Agent>> {
        let agents = self.agents.lock().expect("agent map lock poisoned");
        agents.get(id).map(Arc::clone)
    }

    /// Snapshot the current state of a single agent, or `None` if not found.
    pub fn get_agent_state(&self, id: &str) -> Option<AgentState> {
        let agents = self.agents.lock().expect("agent map lock poisoned");
        agents.get(id).map(|a| a.state())
    }

    /// Snapshot all agents as `(id, state)` pairs, sorted by ID.
    pub fn list_agents(&self) -> Vec<(String, AgentState)> {
        let agents = self.agents.lock().expect("agent map lock poisoned");
        agents.iter().map(|(id, a)| (id.clone(), a.state())).collect()
    }

    /// Remove an agent and its metadata. Returns `false` if not found.
    pub fn remove_agent(&self, id: &str) -> bool {
        let removed = self
            .agents
            .lock()
            .expect("agent map lock poisoned")
            .remove(id)
            .is_some();
        if removed {
            self.agent_meta
                .lock()
                .expect("meta lock poisoned")
                .remove(id);
        }
        removed
    }

    /// Update the RPC endpoint for an agent. Returns `false` if not found.
    pub fn set_rpc(&self, id: &str, url: String) -> bool {
        let agents = self.agents.lock().expect("agent map lock poisoned");
        if let Some(agent) = agents.get(id) {
            agent.set_rpc(url);
            true
        } else {
            false
        }
    }

    /// Apply a Triton config to an agent's RPC endpoint and network. Returns `false` if not found.
    pub fn set_triton(&self, id: &str, config: &TritonConfig) -> bool {
        let agents = self.agents.lock().expect("agent map lock poisoned");
        if let Some(agent) = agents.get(id) {
            agent.set_triton(config);
            true
        } else {
            false
        }
    }

    /// Start the agent's strategy loop. Returns `false` if the agent is not found.
    pub async fn start_agent(&self, id: &str) -> anyhow::Result<bool> {
        let agent = {
            let agents = self.agents.lock().expect("agent map lock poisoned");
            agents.get(id).map(Arc::clone)
        };
        if let Some(agent) = agent {
            agent.start_monitoring().await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Signal an agent's strategy loop to stop. Returns `false` if not found.
    pub fn stop_agent(&self, id: &str) -> bool {
        let agents = self.agents.lock().expect("agent map lock poisoned");
        if let Some(agent) = agents.get(id) {
            agent.stop();
            true
        } else {
            false
        }
    }

    /// Append an action to an agent's log. Returns `false` if not found.
    pub fn record_action(&self, id: &str, action: AgentAction) -> bool {
        let agents = self.agents.lock().expect("agent map lock poisoned");
        if let Some(agent) = agents.get(id) {
            agent.record_action(action);
            true
        } else {
            false
        }
    }

    /// Return all recorded actions for an agent, or `None` if the agent does not exist.
    pub fn get_actions(&self, id: &str) -> Option<Vec<AgentAction>> {
        let agents = self.agents.lock().expect("agent map lock poisoned");
        agents.get(id).map(|a| a.state().actions)
    }

    // -------------------------------------------------------------------------
    // Agent roles
    // -------------------------------------------------------------------------

    /// Assign a new role to an agent. Returns `false` if the agent is not found.
    pub fn set_agent_role(&self, id: &str, role: AgentRole) -> bool {
        let mut meta = self.agent_meta.lock().expect("meta lock poisoned");
        if let Some(m) = meta.get_mut(id) {
            m.role = role;
            true
        } else {
            false
        }
    }

    /// Return the current role of an agent, or `None` if not found.
    pub fn get_agent_role(&self, id: &str) -> Option<AgentRole> {
        let meta = self.agent_meta.lock().expect("meta lock poisoned");
        meta.get(id).map(|m| m.role.clone())
    }

    /// Return the full metadata for an agent, or `None` if not found.
    pub fn get_agent_meta(&self, id: &str) -> Option<AgentMeta> {
        let meta = self.agent_meta.lock().expect("meta lock poisoned");
        meta.get(id).cloned()
    }

    /// Snapshot all agents with their full metadata.
    pub fn list_agents_with_roles(&self) -> Vec<(String, AgentState, AgentMeta)> {
        let agents = self.agents.lock().expect("agent map lock poisoned");
        let meta = self.agent_meta.lock().expect("meta lock poisoned");
        agents
            .iter()
            .map(|(id, agent)| {
                let m = meta.get(id).cloned().unwrap_or_default();
                (id.clone(), agent.state(), m)
            })
            .collect()
    }

    // -------------------------------------------------------------------------
    // Messaging
    // -------------------------------------------------------------------------

    /// Deliver a pre-built message to the bus.
    pub fn send_message(&self, msg: AgentMessage) {
        self.message_bus.send(msg);
    }

    /// Return all messages addressed to (or sent by) `agent_id`.
    pub fn get_messages(&self, agent_id: &str) -> Vec<AgentMessage> {
        self.message_bus.get_messages_for(agent_id)
    }

    /// Return every message on the bus (up to the bus capacity).
    pub fn get_all_messages(&self) -> Vec<AgentMessage> {
        self.message_bus.get_all_messages()
    }

    /// Return the direct-message thread between two agents.
    pub fn get_conversation(&self, a: &str, b: &str) -> Vec<AgentMessage> {
        self.message_bus.get_conversation(a, b)
    }

    // -------------------------------------------------------------------------
    // Shared state
    // -------------------------------------------------------------------------

    /// Write a value to the shared store.
    ///
    /// Requires that `changed_by` has `can_modify_shared_state` permission.
    /// Returns `false` if the permission check fails.
    pub fn set_shared_state(&self, key: &str, value: Value, changed_by: &str) -> bool {
        if !self.check_permission(changed_by, |p| p.can_modify_shared_state) {
            return false;
        }
        self.shared_state.set(key.to_string(), value, changed_by.to_string());
        true
    }

    /// Read a single entry from the shared store.
    pub fn get_shared_state(&self, key: &str) -> Option<crate::shared_state::SharedStateEntry> {
        self.shared_state.get(key)
    }

    /// Return a snapshot of the entire shared store.
    pub fn get_all_shared_state(&self) -> HashMap<String, crate::shared_state::SharedStateEntry> {
        self.shared_state.get_all()
    }

    /// Delete a key from the shared store.
    ///
    /// Requires that `changed_by` has `can_modify_shared_state` permission.
    /// Returns `false` if the permission check fails.
    pub fn delete_shared_state(&self, key: &str, changed_by: &str) -> bool {
        if !self.check_permission(changed_by, |p| p.can_modify_shared_state) {
            return false;
        }
        self.shared_state.delete(key, changed_by.to_string());
        true
    }

    /// Return the change-history log for the shared store.
    pub fn get_state_history(&self) -> Vec<crate::shared_state::StateChange> {
        self.shared_state.get_history()
    }

    // -------------------------------------------------------------------------
    // Workflows
    // -------------------------------------------------------------------------

    /// Register a new workflow with the engine.
    pub fn create_workflow(&self, workflow: Workflow) {
        self.workflow_engine.create_workflow(workflow);
    }

    /// Retrieve a workflow by ID.
    pub fn get_workflow(&self, id: &str) -> Option<Workflow> {
        self.workflow_engine.get_workflow(id)
    }

    /// List all registered workflows.
    pub fn list_workflows(&self) -> Vec<Workflow> {
        self.workflow_engine.list_workflows()
    }

    /// Delete a workflow. Returns `false` if the workflow does not exist.
    pub fn delete_workflow(&self, id: &str) -> bool {
        self.workflow_engine.delete_workflow(id)
    }

    /// Assign an agent to a workflow step.
    pub fn assign_workflow_step(&self, workflow_id: &str, step_id: &str, agent_id: &str) -> bool {
        self.workflow_engine.update_workflow(workflow_id, |w| {
            w.assign_step(step_id, agent_id);
        })
    }

    /// Mark a workflow step as in-progress.
    pub fn start_workflow_step(&self, workflow_id: &str, step_id: &str) -> bool {
        self.workflow_engine.update_workflow(workflow_id, |w| {
            w.start_step(step_id);
        })
    }

    /// Mark a workflow step as completed with an output `result`.
    pub fn complete_workflow_step(
        &self,
        workflow_id: &str,
        step_id: &str,
        result: String,
    ) -> bool {
        self.workflow_engine.update_workflow(workflow_id, |w| {
            w.complete_step(step_id, result);
        })
    }

    /// Mark a workflow step as failed with a `reason`.
    pub fn fail_workflow_step(&self, workflow_id: &str, step_id: &str, reason: String) -> bool {
        self.workflow_engine.update_workflow(workflow_id, |w| {
            w.fail_step(step_id, reason);
        })
    }

    /// Return all workflows that have `agent_id` assigned to at least one step.
    pub fn get_agent_workflows(&self, agent_id: &str) -> Vec<Workflow> {
        self.workflow_engine.get_workflows_for_agent(agent_id)
    }

    /// Return workflows that are in `Draft`, `Running`, or `Paused` status.
    pub fn get_active_workflows(&self) -> Vec<Workflow> {
        self.workflow_engine.get_active_workflows()
    }

    // -------------------------------------------------------------------------
    // Collaboration helpers
    // -------------------------------------------------------------------------

    /// Broadcast a message from `from` to all agents.
    ///
    /// Requires that `from` has `can_broadcast` permission. Returns `false` on failure.
    pub fn broadcast(&self, from: &str, msg_type: &str, payload: &str) -> bool {
        if !self.check_permission(from, |p| p.can_broadcast) {
            return false;
        }
        let msg = AgentMessage::broadcast(
            from.to_string(),
            msg_type.to_string(),
            payload.to_string(),
        );
        self.message_bus.send(msg);
        true
    }

    /// Send a direct message from `from` to `to` without a permission check.
    pub fn send_direct(&self, from: &str, to: &str, msg_type: &str, payload: &str) {
        let msg = AgentMessage::direct(
            from.to_string(),
            to.to_string(),
            msg_type.to_string(),
            payload.to_string(),
        );
        self.message_bus.send(msg);
    }

    // -------------------------------------------------------------------------
    // Internals
    // -------------------------------------------------------------------------

    /// Return `true` if `id` has a role that satisfies `check`.
    fn check_permission(
        &self,
        id: &str,
        check: impl Fn(&crate::role::RolePermissions) -> bool,
    ) -> bool {
        let meta = self.agent_meta.lock().expect("meta lock poisoned");
        meta.get(id)
            .map(|m| check(&m.role.permissions()))
            .unwrap_or(false)
    }
}

impl Default for AgentManager {
    fn default() -> Self {
        Self::new()
    }
}
