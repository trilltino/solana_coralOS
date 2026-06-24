use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// A message exchanged between agents.
///
/// `to = None` is a broadcast; `to = Some(id)` is a direct message.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentMessage {
    pub id: String,
    pub from: String,
    pub to: Option<String>,
    /// Short machine-readable label (e.g. `"task-assigned"`, `"data-ready"`).
    pub msg_type: String,
    pub payload: String,
    pub timestamp: DateTime<Utc>,
}

impl AgentMessage {
    /// Construct a broadcast message (no specific recipient).
    pub fn broadcast(from: String, msg_type: String, payload: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            from,
            to: None,
            msg_type,
            payload,
            timestamp: Utc::now(),
        }
    }

    /// Construct a directed message to a specific agent.
    pub fn direct(from: String, to: String, msg_type: String, payload: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            from,
            to: Some(to),
            msg_type,
            payload,
            timestamp: Utc::now(),
        }
    }

    /// Return `true` if this message is visible to `agent_id`.
    ///
    /// A message is visible when it is a broadcast, sent by the agent, or
    /// directly addressed to the agent.
    pub fn is_visible_to(&self, agent_id: &str) -> bool {
        self.to.is_none() || self.from == agent_id || self.to.as_deref() == Some(agent_id)
    }
}

/// In-memory message store shared by all agents.
///
/// Retains the last 1 000 messages. Unread counts are tracked per recipient
/// for direct messages only; broadcasts are assumed to be read on demand.
#[derive(Clone)]
pub struct MessageBus {
    messages: Arc<Mutex<Vec<AgentMessage>>>,
    unread_counts: Arc<Mutex<HashMap<String, usize>>>,
}

impl Default for MessageBus {
    fn default() -> Self {
        Self::new()
    }
}

impl MessageBus {
    pub fn new() -> Self {
        Self {
            messages: Arc::new(Mutex::new(Vec::new())),
            unread_counts: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Add a message to the bus.
    ///
    /// For direct messages the recipient's unread count is incremented.
    /// Broadcasts are stored as-is; recipients poll via [`get_messages_for`].
    pub fn send(&self, msg: AgentMessage) {
        if let Some(ref to) = msg.to {
            *self
                .unread_counts
                .lock()
                .expect("unread lock poisoned")
                .entry(to.clone())
                .or_insert(0) += 1;
        }

        let mut msgs = self.messages.lock().expect("message store lock poisoned");
        msgs.push(msg);
        // Keep the ring-buffer bounded.
        if msgs.len() > 1000 {
            msgs.remove(0);
        }
    }

    /// Return all messages visible to `agent_id` (broadcasts + direct).
    pub fn get_messages_for(&self, agent_id: &str) -> Vec<AgentMessage> {
        self.messages
            .lock()
            .expect("message store lock poisoned")
            .iter()
            .filter(|m| m.is_visible_to(agent_id))
            .cloned()
            .collect()
    }

    /// Return every message on the bus (admin / debug view).
    pub fn get_all_messages(&self) -> Vec<AgentMessage> {
        self.messages
            .lock()
            .expect("message store lock poisoned")
            .clone()
    }

    /// Return the number of unread direct messages for `agent_id`.
    pub fn get_unread_count(&self, agent_id: &str) -> usize {
        *self
            .unread_counts
            .lock()
            .expect("unread lock poisoned")
            .get(agent_id)
            .unwrap_or(&0)
    }

    /// Reset the unread counter for `agent_id`.
    pub fn clear_unread(&self, agent_id: &str) {
        self.unread_counts
            .lock()
            .expect("unread lock poisoned")
            .remove(agent_id);
    }

    /// Return the direct-message thread between `agent_a` and `agent_b`.
    pub fn get_conversation(&self, agent_a: &str, agent_b: &str) -> Vec<AgentMessage> {
        self.messages
            .lock()
            .expect("message store lock poisoned")
            .iter()
            .filter(|m| {
                (m.from == agent_a && m.to.as_deref() == Some(agent_b))
                    || (m.from == agent_b && m.to.as_deref() == Some(agent_a))
            })
            .cloned()
            .collect()
    }
}
