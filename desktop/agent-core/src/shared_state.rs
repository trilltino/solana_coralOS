use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// A record of a single write or delete on the shared key-value store.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StateChange {
    pub key: String,
    pub old_value: Option<Value>,
    /// `Value::Null` indicates a deletion.
    pub new_value: Value,
    pub timestamp: DateTime<Utc>,
    pub changed_by: String,
}

/// A versioned value in the shared key-value store.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SharedStateEntry {
    pub value: Value,
    pub last_modified: DateTime<Utc>,
    pub modified_by: String,
    /// Monotonically increasing write counter, starting at 1.
    pub version: u64,
}

/// Shared key-value store with a bounded change-history log.
///
/// All agents can read; writes are permission-gated by [`AgentManager`].
/// History is capped at 500 entries to bound memory use.
#[derive(Clone)]
pub struct SharedState {
    store: Arc<Mutex<HashMap<String, SharedStateEntry>>>,
    history: Arc<Mutex<Vec<StateChange>>>,
}

impl Default for SharedState {
    fn default() -> Self {
        Self::new()
    }
}

impl SharedState {
    pub fn new() -> Self {
        Self {
            store: Arc::new(Mutex::new(HashMap::new())),
            history: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Write `value` to `key`, creating or updating the entry.
    pub fn set(&self, key: String, value: Value, changed_by: String) {
        let mut store = self.store.lock().expect("shared state lock poisoned");

        let old_value = store.get(&key).map(|e| e.value.clone());
        let version = store.get(&key).map_or(1, |e| e.version + 1);

        store.insert(
            key.clone(),
            SharedStateEntry {
                value: value.clone(),
                last_modified: Utc::now(),
                modified_by: changed_by.clone(),
                version,
            },
        );
        drop(store);

        self.push_history(StateChange {
            key,
            old_value,
            new_value: value,
            timestamp: Utc::now(),
            changed_by,
        });
    }

    /// Read a single entry.
    pub fn get(&self, key: &str) -> Option<SharedStateEntry> {
        self.store
            .lock()
            .expect("shared state lock poisoned")
            .get(key)
            .cloned()
    }

    /// Return a full snapshot of the store.
    pub fn get_all(&self) -> HashMap<String, SharedStateEntry> {
        self.store.lock().expect("shared state lock poisoned").clone()
    }

    /// Remove `key` from the store and record the deletion in history.
    ///
    /// No-op if the key does not exist.
    pub fn delete(&self, key: &str, changed_by: String) {
        let old_value = {
            let mut store = self.store.lock().expect("shared state lock poisoned");
            store.remove(key).map(|e| e.value)
        };

        if let Some(old) = old_value {
            self.push_history(StateChange {
                key: key.to_string(),
                old_value: Some(old),
                new_value: Value::Null,
                timestamp: Utc::now(),
                changed_by,
            });
        }
    }

    /// Return the bounded change-history log (newest last).
    pub fn get_history(&self) -> Vec<StateChange> {
        self.history.lock().expect("history lock poisoned").clone()
    }

    /// Return all keys currently in the store.
    pub fn get_keys(&self) -> Vec<String> {
        self.store
            .lock()
            .expect("shared state lock poisoned")
            .keys()
            .cloned()
            .collect()
    }

    fn push_history(&self, change: StateChange) {
        let mut history = self.history.lock().expect("history lock poisoned");
        history.push(change);
        if history.len() > 500 {
            history.remove(0);
        }
    }
}
