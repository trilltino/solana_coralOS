use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Execution state of a single workflow step.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum StepStatus {
    Pending,
    Assigned,
    InProgress,
    Completed,
    Failed,
    Skipped,
}

impl Default for StepStatus {
    fn default() -> Self {
        StepStatus::Pending
    }
}

impl std::fmt::Display for StepStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StepStatus::Pending => write!(f, "pending"),
            StepStatus::Assigned => write!(f, "assigned"),
            StepStatus::InProgress => write!(f, "in_progress"),
            StepStatus::Completed => write!(f, "completed"),
            StepStatus::Failed => write!(f, "failed"),
            StepStatus::Skipped => write!(f, "skipped"),
        }
    }
}

/// Overall lifecycle status of a [`Workflow`].
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum WorkflowStatus {
    Draft,
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

impl Default for WorkflowStatus {
    fn default() -> Self {
        WorkflowStatus::Draft
    }
}

impl std::fmt::Display for WorkflowStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkflowStatus::Draft => write!(f, "draft"),
            WorkflowStatus::Running => write!(f, "running"),
            WorkflowStatus::Paused => write!(f, "paused"),
            WorkflowStatus::Completed => write!(f, "completed"),
            WorkflowStatus::Failed => write!(f, "failed"),
            WorkflowStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// A single node in a workflow DAG.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub id: String,
    pub name: String,
    pub description: String,
    pub status: StepStatus,
    /// Agent that owns execution of this step (`None` = unassigned).
    pub assigned_to: Option<String>,
    /// IDs of steps that must be [`StepStatus::Completed`] before this one is ready.
    pub dependencies: Vec<String>,
    /// Output written by the agent that completed this step.
    pub result: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    /// Optional wall-clock deadline in seconds; enforcement is left to the engine.
    pub timeout_secs: Option<u64>,
}

impl WorkflowStep {
    /// Create a new pending step.
    pub fn new(id: &str, name: &str, description: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            description: description.to_string(),
            status: StepStatus::Pending,
            assigned_to: None,
            dependencies: Vec::new(),
            result: None,
            started_at: None,
            completed_at: None,
            timeout_secs: None,
        }
    }

    /// Builder: pre-assign an agent and transition to [`StepStatus::Assigned`].
    pub fn with_assignee(mut self, agent_id: &str) -> Self {
        self.assigned_to = Some(agent_id.to_string());
        self.status = StepStatus::Assigned;
        self
    }

    /// Builder: add a dependency on another step's ID.
    pub fn depends_on(mut self, step_id: &str) -> Self {
        self.dependencies.push(step_id.to_string());
        self
    }
}

/// A named, ordered collection of [`WorkflowStep`]s with dependency edges.
///
/// Steps run when all their dependencies reach [`StepStatus::Completed`].
/// [`Workflow::get_ready_steps`] returns the set that are unblocked right now.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Workflow {
    pub id: String,
    pub name: String,
    pub description: String,
    pub status: WorkflowStatus,
    pub steps: Vec<WorkflowStep>,
    /// Index of the next linear step (used for simple sequential workflows).
    pub current_step: usize,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: String,
    /// All agent IDs that have been assigned at least one step.
    pub assigned_agents: Vec<String>,
    /// Scheduling priority in the range 1–10 (higher = more important).
    pub priority: u8,
    pub tags: Vec<String>,
}

impl Workflow {
    /// Create a new workflow in [`WorkflowStatus::Draft`] with no steps.
    pub fn new(id: &str, name: &str, description: &str, created_by: &str) -> Self {
        let now = Utc::now();
        Self {
            id: id.to_string(),
            name: name.to_string(),
            description: description.to_string(),
            status: WorkflowStatus::Draft,
            steps: Vec::new(),
            current_step: 0,
            created_at: now,
            updated_at: now,
            created_by: created_by.to_string(),
            assigned_agents: Vec::new(),
            priority: 5,
            tags: Vec::new(),
        }
    }

    /// Append a step and update `updated_at`.
    pub fn add_step(&mut self, step: WorkflowStep) {
        self.steps.push(step);
        self.updated_at = Utc::now();
    }

    /// Pre-built workflow: Solana Pay checkout (create URL → wait → validate).
    pub fn solana_pay_checkout(recipient: &str, amount: u64, _label: &str) -> Self {
        let mut wf = Self::new(
            &format!("pay-checkout-{}", uuid::Uuid::new_v4()),
            "Solana Pay Checkout",
            &format!("Checkout of {} lamports to {}", amount, recipient),
            "solana-pay-agent",
        );
        wf.add_step(
            WorkflowStep::new("create_url", "Create Transfer URL", "Generate the solana: URI")
                .with_assignee("pay-agent-transfer"),
        );
        wf.add_step(
            WorkflowStep::new("wait_payment", "Wait for Payment", "Poll until tx is detected")
                .with_assignee("pay-agent-transfer")
                .depends_on("create_url"),
        );
        wf.add_step(
            WorkflowStep::new("validate", "Validate Transfer", "Confirm on-chain")
                .with_assignee("pay-agent-transfer")
                .depends_on("wait_payment"),
        );
        wf
    }

    /// Pre-built workflow: x402 API call (request → parse 402 → pay → retry → verify).
    pub fn x402_api_call(endpoint: &str, budget: u64) -> Self {
        let mut wf = Self::new(
            &format!("x402-call-{}", uuid::Uuid::new_v4()),
            "x402 API Payment",
            &format!("Call {} with max budget {} lamports", endpoint, budget),
            "pay-agent-payment",
        );
        wf.add_step(
            WorkflowStep::new("request", "Send Request", "Initial GET to endpoint")
                .with_assignee("pay-agent-payment"),
        );
        wf.add_step(
            WorkflowStep::new("parse_402", "Parse 402", "Extract payment challenge")
                .with_assignee("pay-agent-payment")
                .depends_on("request"),
        );
        wf.add_step(
            WorkflowStep::new("build_payment", "Build Payment", "Construct stablecoin tx")
                .with_assignee("pay-agent-payment")
                .depends_on("parse_402"),
        );
        wf.add_step(
            WorkflowStep::new("retry", "Retry with Payment", "Resend with payment proof")
                .with_assignee("pay-agent-payment")
                .depends_on("build_payment"),
        );
        wf.add_step(
            WorkflowStep::new("verify", "Verify Settlement", "Confirm on-chain")
                .with_assignee("pay-agent-payment")
                .depends_on("retry"),
        );
        wf
    }

    /// Return steps whose dependencies are all completed and that are still pending.
    pub fn get_ready_steps(&self) -> Vec<&WorkflowStep> {
        self.steps
            .iter()
            .filter(|s| {
                s.status == StepStatus::Pending
                    && s.dependencies.iter().all(|dep_id| {
                        self.steps
                            .iter()
                            .any(|d| d.id == *dep_id && d.status == StepStatus::Completed)
                    })
            })
            .collect()
    }

    /// Assign `agent_id` to a step and record the agent in `assigned_agents`.
    ///
    /// Returns `false` if `step_id` does not exist.
    pub fn assign_step(&mut self, step_id: &str, agent_id: &str) -> bool {
        if let Some(step) = self.steps.iter_mut().find(|s| s.id == step_id) {
            step.assigned_to = Some(agent_id.to_string());
            step.status = StepStatus::Assigned;
            if !self.assigned_agents.iter().any(|a| a == agent_id) {
                self.assigned_agents.push(agent_id.to_string());
            }
            self.updated_at = Utc::now();
            true
        } else {
            false
        }
    }

    /// Mark a step as in-progress. Returns `false` if `step_id` does not exist.
    pub fn start_step(&mut self, step_id: &str) -> bool {
        if let Some(step) = self.steps.iter_mut().find(|s| s.id == step_id) {
            step.status = StepStatus::InProgress;
            step.started_at = Some(Utc::now());
            self.updated_at = Utc::now();
            true
        } else {
            false
        }
    }

    /// Mark a step as completed with an output string.
    ///
    /// Also advances `current_step` if this was the current index, and
    /// transitions the workflow to [`WorkflowStatus::Completed`] if all steps
    /// are done. Returns `false` if `step_id` does not exist.
    pub fn complete_step(&mut self, step_id: &str, result: String) -> bool {
        if let Some(step) = self.steps.iter_mut().find(|s| s.id == step_id) {
            step.status = StepStatus::Completed;
            step.result = Some(result);
            step.completed_at = Some(Utc::now());
            self.updated_at = Utc::now();
        } else {
            return false;
        }

        if let Some(pos) = self.steps.iter().position(|s| s.id == step_id) {
            if pos == self.current_step {
                self.current_step += 1;
            }
        }

        if self.steps.iter().all(|s| s.status == StepStatus::Completed) {
            self.status = WorkflowStatus::Completed;
        }

        true
    }

    /// Mark a step (and the whole workflow) as failed. Returns `false` if `step_id` does not exist.
    pub fn fail_step(&mut self, step_id: &str, reason: String) -> bool {
        if let Some(step) = self.steps.iter_mut().find(|s| s.id == step_id) {
            step.status = StepStatus::Failed;
            step.result = Some(reason);
            self.status = WorkflowStatus::Failed;
            self.updated_at = Utc::now();
            true
        } else {
            false
        }
    }

    /// Return the percentage of steps that are completed (0–100).
    pub fn progress_pct(&self) -> u8 {
        if self.steps.is_empty() {
            return 0;
        }
        let completed = self
            .steps
            .iter()
            .filter(|s| s.status == StepStatus::Completed)
            .count();
        ((completed as f32 / self.steps.len() as f32) * 100.0) as u8
    }
}
