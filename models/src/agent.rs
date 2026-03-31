use serde::{Deserialize, Serialize};
use std::hash::Hash;

/// Specialist agents available for agent routing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentSpecialist {
    SqlExpert,
    DataAnalyst,
    SchemaArchitect,
}

impl AgentSpecialist {
    pub fn variant_name(&self) -> &'static str {
        match self {
            AgentSpecialist::SqlExpert => "SqlExpert",
            AgentSpecialist::DataAnalyst => "DataAnalyst",
            AgentSpecialist::SchemaArchitect => "SchemaArchitect",
        }
    }
}

/// Request for routing a query to the appropriate specialist agent.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentRoutingRequest {
    pub query_text: String,
    pub database_context: String,
    pub user_intent: Option<String>,
}

/// Response containing the routing decision.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentRoutingResponse {
    pub specialist: AgentSpecialist,
    pub confidence: f32,
    pub reasoning: String,
}

/// Capability descriptor for a specialist agent.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentCapability {
    pub specialist: AgentSpecialist,
    pub description: String,
    pub example_queries: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_specialist_variants() {
        let specialists = [
            AgentSpecialist::SqlExpert,
            AgentSpecialist::DataAnalyst,
            AgentSpecialist::SchemaArchitect,
        ];
        assert_eq!(specialists.len(), 3);
    }

    #[test]
    fn agent_routing_request_fields() {
        let request = AgentRoutingRequest {
            query_text: "SELECT * FROM users".to_string(),
            database_context: "sqlite:./db.sqlite".to_string(),
            user_intent: Some("Find all users".to_string()),
        };
        assert_eq!(request.query_text, "SELECT * FROM users");
        assert_eq!(request.database_context, "sqlite:./db.sqlite");
        assert_eq!(request.user_intent, Some("Find all users".to_string()));
    }

    #[test]
    fn agent_routing_response_fields() {
        let response = AgentRoutingResponse {
            specialist: AgentSpecialist::SqlExpert,
            confidence: 0.95,
            reasoning: "Query is a standard SELECT statement".to_string(),
        };
        assert_eq!(response.specialist, AgentSpecialist::SqlExpert);
        assert_eq!(response.confidence, 0.95);
        assert_eq!(response.reasoning, "Query is a standard SELECT statement");
    }

    #[test]
    fn agent_capability_fields() {
        let capability = AgentCapability {
            specialist: AgentSpecialist::DataAnalyst,
            description: "Analyzes data and generates insights".to_string(),
            example_queries: vec![
                "What is the average order value?".to_string(),
                "Show me monthly trends".to_string(),
            ],
        };
        assert_eq!(capability.specialist, AgentSpecialist::DataAnalyst);
        assert_eq!(
            capability.description,
            "Analyzes data and generates insights"
        );
        assert_eq!(capability.example_queries.len(), 2);
    }
}
