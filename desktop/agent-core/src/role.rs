use serde::{Deserialize, Serialize};

/// Roles that can be assigned to an agent to control what it is allowed to do.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AgentRole {
    Leader,
    Worker,
    Monitor,
    Analyst,
    Trader,
    Coordinator,
}

impl Default for AgentRole {
    fn default() -> Self {
        AgentRole::Worker
    }
}

impl std::fmt::Display for AgentRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentRole::Leader => write!(f, "leader"),
            AgentRole::Worker => write!(f, "worker"),
            AgentRole::Monitor => write!(f, "monitor"),
            AgentRole::Analyst => write!(f, "analyst"),
            AgentRole::Trader => write!(f, "trader"),
            AgentRole::Coordinator => write!(f, "coordinator"),
        }
    }
}

/// Capability flags derived from an agent's [`AgentRole`].
///
/// Obtain via [`AgentRole::permissions`] or `RolePermissions::from(role)`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RolePermissions {
    pub can_broadcast: bool,
    pub can_assign_tasks: bool,
    pub can_modify_shared_state: bool,
    pub can_start_workflows: bool,
    pub can_stop_other_agents: bool,
    pub can_view_all_messages: bool,
    pub can_initiate_payments: bool,
    pub can_create_payment_requests: bool,
}

impl Default for RolePermissions {
    fn default() -> Self {
        Self {
            can_broadcast: false,
            can_assign_tasks: false,
            can_modify_shared_state: false,
            can_start_workflows: false,
            can_stop_other_agents: false,
            can_view_all_messages: false,
            can_initiate_payments: false,
            can_create_payment_requests: false,
        }
    }
}

impl From<AgentRole> for RolePermissions {
    fn from(role: AgentRole) -> Self {
        match role {
            AgentRole::Leader => Self {
                can_broadcast: true,
                can_assign_tasks: true,
                can_modify_shared_state: true,
                can_start_workflows: true,
                can_stop_other_agents: true,
                can_view_all_messages: true,
                can_initiate_payments: true,
                can_create_payment_requests: true,
            },
            AgentRole::Coordinator => Self {
                can_broadcast: true,
                can_assign_tasks: true,
                can_modify_shared_state: true,
                can_start_workflows: true,
                can_stop_other_agents: false,
                can_view_all_messages: true,
                can_initiate_payments: true,
                can_create_payment_requests: true,
            },
            AgentRole::Worker => Self {
                can_modify_shared_state: true,
                ..Self::default()
            },
            AgentRole::Monitor => Self {
                can_broadcast: true,
                can_view_all_messages: true,
                ..Self::default()
            },
            AgentRole::Analyst => Self {
                can_modify_shared_state: true,
                ..Self::default()
            },
            AgentRole::Trader => Self {
                can_modify_shared_state: true,
                can_initiate_payments: true,
                can_create_payment_requests: true,
                ..Self::default()
            },
        }
    }
}

impl AgentRole {
    /// Return the capability flags for this role.
    pub fn permissions(&self) -> RolePermissions {
        RolePermissions::from(self.clone())
    }

    /// Return a human-readable description of this role.
    pub fn description(&self) -> &'static str {
        match self {
            AgentRole::Leader => {
                "Full control: can assign tasks, broadcast, manage workflows, and stop agents"
            }
            AgentRole::Worker => "Executes assigned tasks and can update shared state",
            AgentRole::Monitor => "Observes agent health and can broadcast alerts",
            AgentRole::Analyst => "Processes data and writes insights to shared state",
            AgentRole::Trader => "Executes trades and records transaction data",
            AgentRole::Coordinator => {
                "Manages workflows and task assignment without agent stop rights"
            }
        }
    }
}
