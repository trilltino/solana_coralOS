use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::workflow::{Workflow, WorkflowStatus};

/// In-memory store and mutation facade for [`Workflow`]s.
///
/// All mutations go through [`update_workflow`], which holds the lock for the
/// duration of the callback, making multi-field updates atomic.
#[derive(Clone)]
pub struct WorkflowEngine {
    workflows: Arc<Mutex<HashMap<String, Workflow>>>,
}

impl Default for WorkflowEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkflowEngine {
    pub fn new() -> Self {
        Self {
            workflows: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Register a new workflow. Overwrites any existing workflow with the same ID.
    pub fn create_workflow(&self, workflow: Workflow) {
        self.workflows
            .lock()
            .expect("workflow store lock poisoned")
            .insert(workflow.id.clone(), workflow);
    }

    /// Retrieve a cloned snapshot of a workflow by ID.
    pub fn get_workflow(&self, id: &str) -> Option<Workflow> {
        self.workflows
            .lock()
            .expect("workflow store lock poisoned")
            .get(id)
            .cloned()
    }

    /// Return snapshots of all registered workflows.
    pub fn list_workflows(&self) -> Vec<Workflow> {
        self.workflows
            .lock()
            .expect("workflow store lock poisoned")
            .values()
            .cloned()
            .collect()
    }

    /// Apply `f` to a mutable workflow while holding the store lock.
    ///
    /// Returns `false` if no workflow with `id` exists.
    pub fn update_workflow(&self, id: &str, f: impl FnOnce(&mut Workflow)) -> bool {
        let mut wfs = self.workflows.lock().expect("workflow store lock poisoned");
        if let Some(wf) = wfs.get_mut(id) {
            f(wf);
            true
        } else {
            false
        }
    }

    /// Delete a workflow. Returns `false` if it does not exist.
    pub fn delete_workflow(&self, id: &str) -> bool {
        self.workflows
            .lock()
            .expect("workflow store lock poisoned")
            .remove(id)
            .is_some()
    }

    /// Return workflows that have `agent_id` in their `assigned_agents` list.
    pub fn get_workflows_for_agent(&self, agent_id: &str) -> Vec<Workflow> {
        self.workflows
            .lock()
            .expect("workflow store lock poisoned")
            .values()
            .filter(|w| w.assigned_agents.contains(&agent_id.to_string()))
            .cloned()
            .collect()
    }

    /// Return workflows in `Draft`, `Running`, or `Paused` status.
    pub fn get_active_workflows(&self) -> Vec<Workflow> {
        self.workflows
            .lock()
            .expect("workflow store lock poisoned")
            .values()
            .filter(|w| {
                matches!(
                    w.status,
                    WorkflowStatus::Running | WorkflowStatus::Draft | WorkflowStatus::Paused
                )
            })
            .cloned()
            .collect()
    }
}
