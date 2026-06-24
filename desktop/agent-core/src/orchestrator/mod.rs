//! DAG-based workflow orchestration.
//!
//! [`Workflow`] defines a named set of [`WorkflowStep`]s with dependency edges.
//! [`WorkflowEngine`] stores workflows and provides transactional mutation via
//! [`WorkflowEngine::update_workflow`].

pub mod engine;
pub mod workflow;

pub use engine::WorkflowEngine;
pub use workflow::{StepStatus, Workflow, WorkflowStatus, WorkflowStep};
