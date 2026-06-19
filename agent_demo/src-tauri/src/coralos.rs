use agent_core::{AgentAction, AgentState, Workflow};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

/// CoralOS agent status within a session.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CoralAgent {
    pub name: String,
    pub status: String,
    pub description: String,
    pub links: Vec<String>,
}

/// Extended session state with agent list.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionStateExtended {
    pub id: String,
    pub namespace: String,
    pub status: String,
    pub agents: Vec<CoralAgent>,
}

/// Lightweight CoralOS HTTP client.
pub struct CoralOSClient {
    base_url: Arc<Mutex<String>>,
    api_token: Arc<Mutex<String>>,
    client: reqwest::Client,
}

impl CoralOSClient {
    pub fn new(base_url: String, api_token: String) -> Self {
        Self {
            base_url: Arc::new(Mutex::new(base_url)),
            api_token: Arc::new(Mutex::new(api_token)),
            client: reqwest::Client::new(),
        }
    }

    pub fn set_url(&self, url: String) {
        let mut guard = self.base_url.lock().unwrap();
        *guard = url.trim_end_matches('/').to_string();
    }

    pub fn set_token(&self, token: String) {
        let mut guard = self.api_token.lock().unwrap();
        *guard = token;
    }

    fn url(&self) -> String {
        self.base_url.lock().unwrap().clone()
    }

    fn token(&self) -> String {
        self.api_token.lock().unwrap().clone()
    }

    // --- Agents ---

    pub async fn list_agents(&self) -> anyhow::Result<Vec<(String, AgentState)>> {
        let url = format!("{}/api/v1/agents", self.url());
        let resp = self
            .client
            .get(&url)
            .bearer_auth(self.token())
            .send()
            .await?;
        resp.error_for_status_ref()?;
        let agents: Vec<(String, AgentState)> = resp.json().await?;
        Ok(agents)
    }

    pub async fn get_agent(&self, id: &str) -> anyhow::Result<AgentState> {
        let url = format!("{}/api/v1/agents/{}", self.url(), id);
        let resp = self
            .client
            .get(&url)
            .bearer_auth(self.token())
            .send()
            .await?;
        resp.error_for_status_ref()?;
        let agent: AgentState = resp.json().await?;
        Ok(agent)
    }

    pub async fn create_agent(&self, id: &str) -> anyhow::Result<AgentState> {
        let url = format!("{}/api/v1/agents", self.url());
        let resp = self
            .client
            .post(&url)
            .bearer_auth(self.token())
            .json(&serde_json::json!({ "id": id }))
            .send()
            .await?;
        resp.error_for_status_ref()?;
        let agent: AgentState = resp.json().await?;
        Ok(agent)
    }

    pub async fn start_agent(&self, id: &str) -> anyhow::Result<bool> {
        let url = format!("{}/api/v1/agents/{}/start", self.url(), id);
        let resp = self
            .client
            .post(&url)
            .bearer_auth(self.token())
            .send()
            .await?;
        resp.error_for_status_ref()?;
        let result: bool = resp.json().await?;
        Ok(result)
    }

    pub async fn stop_agent(&self, id: &str) -> anyhow::Result<bool> {
        let url = format!("{}/api/v1/agents/{}/stop", self.url(), id);
        let resp = self
            .client
            .post(&url)
            .bearer_auth(self.token())
            .send()
            .await?;
        resp.error_for_status_ref()?;
        let result: bool = resp.json().await?;
        Ok(result)
    }

    pub async fn delete_agent(&self, id: &str) -> anyhow::Result<bool> {
        let url = format!("{}/api/v1/agents/{}", self.url(), id);
        let resp = self
            .client
            .delete(&url)
            .bearer_auth(self.token())
            .send()
            .await?;
        resp.error_for_status_ref()?;
        let result: bool = resp.json().await?;
        Ok(result)
    }

    pub async fn get_agent_actions(&self, id: &str) -> anyhow::Result<Vec<AgentAction>> {
        let url = format!("{}/api/v1/agents/{}/actions", self.url(), id);
        let resp = self
            .client
            .get(&url)
            .bearer_auth(self.token())
            .send()
            .await?;
        resp.error_for_status_ref()?;
        let actions: Vec<AgentAction> = resp.json().await?;
        Ok(actions)
    }

    // --- Workflows ---

    pub async fn list_workflows(&self) -> anyhow::Result<Vec<Workflow>> {
        let url = format!("{}/api/v1/workflows", self.url());
        let resp = self
            .client
            .get(&url)
            .bearer_auth(self.token())
            .send()
            .await?;
        resp.error_for_status_ref()?;
        let workflows: Vec<Workflow> = resp.json().await?;
        Ok(workflows)
    }

    pub async fn get_workflow(&self, id: &str) -> anyhow::Result<Workflow> {
        let url = format!("{}/api/v1/workflows/{}", self.url(), id);
        let resp = self
            .client
            .get(&url)
            .bearer_auth(self.token())
            .send()
            .await?;
        resp.error_for_status_ref()?;
        let workflow: Workflow = resp.json().await?;
        Ok(workflow)
    }

    // --- Legacy session methods mapped to agents ---

    /// List all agents as sessions (legacy compatibility).
    pub async fn list_sessions(&self, namespace: &str) -> anyhow::Result<Vec<SessionStateExtended>> {
        let agents = self.list_agents().await?;
        Ok(agents
            .into_iter()
            .map(|(id, state)| SessionStateExtended {
                id,
                namespace: namespace.to_string(),
                status: if state.is_running {
                    "active".to_string()
                } else {
                    "stopped".to_string()
                },
                agents: vec![CoralAgent {
                    name: state.strategy.clone(),
                    status: if state.is_running {
                        "running".to_string()
                    } else {
                        "stopped".to_string()
                    },
                    description: format!("RPC: {}", state.rpc_endpoint),
                    links: vec![],
                }],
            })
            .collect())
    }

    /// Get single agent as session (legacy compatibility).
    pub async fn get_session(
        &self,
        namespace: &str,
        session_id: &str,
    ) -> anyhow::Result<SessionStateExtended> {
        let state = self.get_agent(session_id).await?;
        Ok(SessionStateExtended {
            id: session_id.to_string(),
            namespace: namespace.to_string(),
            status: if state.is_running {
                "active".to_string()
            } else {
                "stopped".to_string()
            },
            agents: vec![CoralAgent {
                name: state.strategy.clone(),
                status: if state.is_running {
                    "running".to_string()
                } else {
                    "stopped".to_string()
                },
                description: format!("RPC: {}", state.rpc_endpoint),
                links: vec![],
            }],
        })
    }
}
