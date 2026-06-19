//! Workflow management endpoints.
//!
//! | Method | Path                                     | Action                   |
//! |--------|------------------------------------------|--------------------------|
//! | GET    | `/`                                      | List workflows           |
//! | POST   | `/`                                      | Create a workflow        |
//! | GET    | `/:id`                                   | Get a workflow           |
//! | DELETE | `/:id`                                   | Delete a workflow        |
//! | POST   | `/:id/steps/:step_id/assign`             | Assign step to agent     |
//! | POST   | `/:id/steps/:step_id/start`              | Mark step in-progress    |
//! | POST   | `/:id/steps/:step_id/complete`           | Mark step completed      |
//! | POST   | `/:id/steps/:step_id/fail`               | Mark step failed         |

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::AppState;
use agent_core::{Workflow, WorkflowStep};

#[derive(Serialize, Deserialize)]
pub struct CreateWorkflowRequest {
    pub id: String,
    pub name: String,
    pub description: String,
    pub steps: Vec<WorkflowStep>,
    /// Priority in the range 1–10. Values outside this range are clamped.
    pub priority: u8,
    pub created_by: String,
}

#[derive(Serialize, Deserialize)]
pub struct AssignStepRequest {
    pub agent_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct CompleteStepRequest {
    pub result: String,
}

#[derive(Serialize, Deserialize)]
pub struct FailStepRequest {
    pub reason: String,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_workflows).post(create_workflow))
        .route("/:id", get(get_workflow).delete(delete_workflow))
        .route("/:id/steps/:step_id/assign", post(assign_step))
        .route("/:id/steps/:step_id/start", post(start_step))
        .route("/:id/steps/:step_id/complete", post(complete_step))
        .route("/:id/steps/:step_id/fail", post(fail_step))
}

async fn create_workflow(
    State(state): State<AppState>,
    Json(req): Json<CreateWorkflowRequest>,
) -> Result<Json<Workflow>, StatusCode> {
    if req.id.trim().is_empty() || req.name.trim().is_empty() {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let mut workflow = Workflow::new(&req.id, &req.name, &req.description, &req.created_by);
    workflow.priority = req.priority.clamp(1, 10);
    for step in req.steps {
        workflow.add_step(step);
    }
    state.manager.create_workflow(workflow.clone());
    Ok(Json(workflow))
}

async fn list_workflows(State(state): State<AppState>) -> Json<Vec<Workflow>> {
    Json(state.manager.list_workflows())
}

async fn get_workflow(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Workflow>, StatusCode> {
    state
        .manager
        .get_workflow(&id)
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn delete_workflow(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    if state.manager.delete_workflow(&id) {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

async fn assign_step(
    State(state): State<AppState>,
    Path((workflow_id, step_id)): Path<(String, String)>,
    Json(req): Json<AssignStepRequest>,
) -> Result<Json<bool>, StatusCode> {
    Ok(Json(
        state
            .manager
            .assign_workflow_step(&workflow_id, &step_id, &req.agent_id),
    ))
}

async fn start_step(
    State(state): State<AppState>,
    Path((workflow_id, step_id)): Path<(String, String)>,
) -> Result<Json<bool>, StatusCode> {
    Ok(Json(
        state.manager.start_workflow_step(&workflow_id, &step_id),
    ))
}

async fn complete_step(
    State(state): State<AppState>,
    Path((workflow_id, step_id)): Path<(String, String)>,
    Json(req): Json<CompleteStepRequest>,
) -> Result<Json<bool>, StatusCode> {
    Ok(Json(
        state
            .manager
            .complete_workflow_step(&workflow_id, &step_id, req.result),
    ))
}

async fn fail_step(
    State(state): State<AppState>,
    Path((workflow_id, step_id)): Path<(String, String)>,
    Json(req): Json<FailStepRequest>,
) -> Result<Json<bool>, StatusCode> {
    Ok(Json(
        state
            .manager
            .fail_workflow_step(&workflow_id, &step_id, req.reason),
    ))
}
