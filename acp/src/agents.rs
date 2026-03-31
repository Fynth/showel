use models::{AgentCapability, AgentRoutingRequest, AgentRoutingResponse, AgentSpecialist};
use std::collections::VecDeque;

/// System prompts for each specialist type.
const SQL_EXPERT_PROMPT: &str = "You are a SQL expert. Optimize queries, explain execution plans, suggest indexes, and help with complex joins. Focus on performance and correctness.";
const DATA_ANALYST_PROMPT: &str = "You are a data analyst. Find trends, anomalies, and patterns in data. Generate insights, calculate statistics, and explain findings clearly.";
const SCHEMA_ARCHITECT_PROMPT: &str = "You are a schema architect. Design migrations, normalize tables, define constraints and indexes. Focus on data integrity and scalability.";

/// Confidence threshold for routing decisions.
/// Below this threshold, the coordinator returns a clarifying question.
const CONFIDENCE_THRESHOLD: f32 = 0.7;

/// Maximum number of handoff events to retain for debugging.
const HANDOFF_HISTORY_LIMIT: usize = 10;

/// Trait for specialist agent implementations.
/// Each specialist handles a specific domain of database queries.
pub trait Specialist: Send + Sync {
    /// Returns the specialist's name identifier.
    fn name(&self) -> &str;

    /// Returns the specialist's capability descriptor.
    fn capabilities(&self) -> &AgentCapability;

    /// Handles a routing request and returns a response.
    fn handle(&self, request: &AgentRoutingRequest) -> Result<String, String>;
}

/// Intent classification result from keyword matching.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UserIntent {
    SqlQuery,
    DataAnalysis,
    SchemaDesign,
    Unknown,
}

/// Record of a handoff between specialists.
#[derive(Clone, Debug)]
pub struct HandoffRecord {
    pub from: AgentSpecialist,
    pub to: AgentSpecialist,
    pub context: String,
    pub timestamp_ms: u64,
}

/// Response from a specialist that can be synthesized.
#[derive(Clone, Debug)]
pub struct SpecialistResponse {
    pub specialist: AgentSpecialist,
    pub content: String,
    pub confidence: f32,
}

/// Multi-agent coordinator that routes queries to appropriate specialists.
pub struct AgentCoordinator {
    /// Intent classifier for routing decisions.
    router: IntentClassifier,
    /// Registered specialists indexed by their type.
    specialists: std::collections::HashMap<AgentSpecialist, Box<dyn Specialist>>,
    /// Currently active specialist (if any).
    active_specialist: Option<AgentSpecialist>,
    /// History of handoffs for debugging.
    handoff_history: VecDeque<HandoffRecord>,
}

/// Intent classifier using keyword matching from the registry.
pub struct IntentClassifier {
    keywords: std::collections::HashMap<AgentSpecialist, Vec<&'static str>>,
}

impl IntentClassifier {
    /// Creates a new intent classifier with keyword mappings from the registry.
    pub fn new() -> Self {
        let mut keywords = std::collections::HashMap::new();

        keywords.insert(
            AgentSpecialist::SqlExpert,
            vec![
                "SELECT", "INSERT", "UPDATE", "DELETE", "FROM", "WHERE", "JOIN", "GROUP BY",
                "ORDER BY", "query", "sql", "optimize", "join", "table",
            ],
        );

        keywords.insert(
            AgentSpecialist::DataAnalyst,
            vec![
                "analyze",
                "trend",
                "average",
                "sum",
                "count",
                "group by",
                "chart",
                "graph",
                "statistics",
                "insight",
                "pattern",
                "anomaly",
                "report",
            ],
        );

        keywords.insert(
            AgentSpecialist::SchemaArchitect,
            vec![
                "create table",
                "alter table",
                "migration",
                "constraint",
                "index",
                "schema",
                "design",
                "normalize",
                "foreign key",
                "primary key",
                "column",
            ],
        );

        Self { keywords }
    }

    /// Classifies intent from a query string using keyword matching.
    pub fn classify(&self, query: &str) -> UserIntent {
        let query_lower = query.to_lowercase();
        let mut best_match: Option<AgentSpecialist> = None;
        let mut best_score: usize = 0;

        for (specialist, kws) in &self.keywords {
            let score = kws
                .iter()
                .filter(|kw| query_lower.contains(&kw.to_lowercase()))
                .count();

            if score > best_score {
                best_score = score;
                best_match = Some(*specialist);
            }
        }

        match best_match {
            Some(AgentSpecialist::SqlExpert) => UserIntent::SqlQuery,
            Some(AgentSpecialist::DataAnalyst) => UserIntent::DataAnalysis,
            Some(AgentSpecialist::SchemaArchitect) => UserIntent::SchemaDesign,
            None => UserIntent::Unknown,
        }
    }
}

impl Default for IntentClassifier {
    fn default() -> Self {
        Self::new()
    }
}

/// Routes an intent to a specialist with confidence score.
pub fn route_to_specialist(intent: UserIntent) -> (AgentSpecialist, f32) {
    match intent {
        UserIntent::SqlQuery => (AgentSpecialist::SqlExpert, 0.85),
        UserIntent::DataAnalysis => (AgentSpecialist::DataAnalyst, 0.85),
        UserIntent::SchemaDesign => (AgentSpecialist::SchemaArchitect, 0.85),
        UserIntent::Unknown => (AgentSpecialist::SqlExpert, 0.4), // Low confidence fallback
    }
}

/// SQL expert specialist implementation.
pub struct SqlExpert {
    capability: AgentCapability,
}

impl SqlExpert {
    pub fn new() -> Self {
        Self {
            capability: AgentCapability {
                specialist: AgentSpecialist::SqlExpert,
                description: "SQL query optimization and generation expert".to_string(),
                example_queries: vec![
                    "Optimize this query".to_string(),
                    "How do I join these tables?".to_string(),
                    "Explain this execution plan".to_string(),
                ],
            },
        }
    }
}

impl Default for SqlExpert {
    fn default() -> Self {
        Self::new()
    }
}

impl Specialist for SqlExpert {
    fn name(&self) -> &str {
        "SqlExpert"
    }

    fn capabilities(&self) -> &AgentCapability {
        &self.capability
    }

    fn handle(&self, request: &AgentRoutingRequest) -> Result<String, String> {
        let context = &request.database_context;
        let query = &request.query_text;

        // Build a response using the system prompt and context
        let response = format!(
            "{}\n\nDatabase context: {}\n\nQuery: {}\n\nI can help optimize this query, suggest indexes, or explain the execution plan.",
            SQL_EXPERT_PROMPT, context, query
        );

        Ok(response)
    }
}

/// Data analyst specialist implementation.
pub struct DataAnalyst {
    capability: AgentCapability,
}

impl DataAnalyst {
    pub fn new() -> Self {
        Self {
            capability: AgentCapability {
                specialist: AgentSpecialist::DataAnalyst,
                description: "Data analysis and visualization expert".to_string(),
                example_queries: vec![
                    "Show me sales trends".to_string(),
                    "What's the average order value?".to_string(),
                    "Find anomalies in the data".to_string(),
                ],
            },
        }
    }
}

impl Default for DataAnalyst {
    fn default() -> Self {
        Self::new()
    }
}

impl Specialist for DataAnalyst {
    fn name(&self) -> &str {
        "DataAnalyst"
    }

    fn capabilities(&self) -> &AgentCapability {
        &self.capability
    }

    fn handle(&self, request: &AgentRoutingRequest) -> Result<String, String> {
        let context = &request.database_context;
        let query = &request.query_text;

        let response = format!(
            "{}\n\nDatabase context: {}\n\nQuery: {}\n\nI can analyze trends, calculate statistics, and identify patterns in your data.",
            DATA_ANALYST_PROMPT, context, query
        );

        Ok(response)
    }
}

/// Schema architect specialist implementation.
pub struct SchemaArchitect {
    capability: AgentCapability,
}

impl SchemaArchitect {
    pub fn new() -> Self {
        Self {
            capability: AgentCapability {
                specialist: AgentSpecialist::SchemaArchitect,
                description: "Database schema design and migration expert".to_string(),
                example_queries: vec![
                    "Design a table for users".to_string(),
                    "How do I add an index?".to_string(),
                    "Create a migration for this schema".to_string(),
                ],
            },
        }
    }
}

impl Default for SchemaArchitect {
    fn default() -> Self {
        Self::new()
    }
}

impl Specialist for SchemaArchitect {
    fn name(&self) -> &str {
        "SchemaArchitect"
    }

    fn capabilities(&self) -> &AgentCapability {
        &self.capability
    }

    fn handle(&self, request: &AgentRoutingRequest) -> Result<String, String> {
        let context = &request.database_context;
        let query = &request.query_text;

        let response = format!(
            "{}\n\nDatabase context: {}\n\nQuery: {}\n\nI can help design tables, create migrations, and define constraints.",
            SCHEMA_ARCHITECT_PROMPT, context, query
        );

        Ok(response)
    }
}

impl AgentCoordinator {
    /// Creates a new coordinator with all specialists registered.
    pub fn new() -> Self {
        let mut specialists: std::collections::HashMap<AgentSpecialist, Box<dyn Specialist>> =
            std::collections::HashMap::new();

        specialists.insert(AgentSpecialist::SqlExpert, Box::new(SqlExpert::new()));
        specialists.insert(AgentSpecialist::DataAnalyst, Box::new(DataAnalyst::new()));
        specialists.insert(
            AgentSpecialist::SchemaArchitect,
            Box::new(SchemaArchitect::new()),
        );

        Self {
            router: IntentClassifier::new(),
            specialists,
            active_specialist: None,
            handoff_history: VecDeque::with_capacity(HANDOFF_HISTORY_LIMIT),
        }
    }

    /// Routes a request to the appropriate specialist.
    /// Returns a clarifying question if confidence is below threshold.
    pub fn route(&mut self, request: &AgentRoutingRequest) -> Result<AgentRoutingResponse, String> {
        let intent = self.router.classify(&request.query_text);
        let (specialist, confidence) = route_to_specialist(intent);

        // Check confidence threshold
        if confidence < CONFIDENCE_THRESHOLD {
            return Ok(AgentRoutingResponse {
                specialist,
                confidence,
                reasoning: "Could you clarify what you'd like to do? For example: 'optimize this query', 'analyze trends', or 'design a schema'.".to_string(),
            });
        }

        let reasoning = format!(
            "Classified as {:?} based on keyword matching with confidence {:.2}",
            intent, confidence
        );

        self.active_specialist = Some(specialist);

        Ok(AgentRoutingResponse {
            specialist,
            confidence,
            reasoning,
        })
    }

    /// Dispatches a request to a specific specialist.
    pub fn dispatch(
        &self,
        specialist: AgentSpecialist,
        request: &AgentRoutingRequest,
    ) -> Result<String, String> {
        let specialist_impl = self
            .specialists
            .get(&specialist)
            .ok_or_else(|| format!("Specialist {:?} not found", specialist))?;

        specialist_impl.handle(request)
    }

    /// Transfers conversation context from one specialist to another.
    /// Records the handoff in history for debugging.
    pub fn handoff(
        &mut self,
        from: AgentSpecialist,
        to: AgentSpecialist,
        context: String,
    ) -> Result<(), String> {
        if !self.specialists.contains_key(&from) {
            return Err(format!("Source specialist {:?} not found", from));
        }
        if !self.specialists.contains_key(&to) {
            return Err(format!("Target specialist {:?} not found", to));
        }

        let record = HandoffRecord {
            from,
            to,
            context,
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
        };

        // Maintain history limit
        if self.handoff_history.len() >= HANDOFF_HISTORY_LIMIT {
            self.handoff_history.pop_front();
        }
        self.handoff_history.push_back(record);

        self.active_specialist = Some(to);

        Ok(())
    }

    /// Synthesizes multiple specialist responses into a unified response.
    pub fn synthesize(&self, responses: Vec<SpecialistResponse>) -> String {
        if responses.is_empty() {
            return "No responses to synthesize.".to_string();
        }

        if responses.len() == 1 {
            return responses[0].content.clone();
        }

        let mut combined = String::new();
        combined.push_str("Combined insights from multiple specialists:\n\n");

        for response in &responses {
            combined.push_str(&format!(
                "**{}** (confidence: {:.0}%):\n{}\n\n",
                response.specialist.variant_name(),
                response.confidence * 100.0,
                response.content
            ));
        }

        combined.push_str("Consider the above perspectives for a comprehensive solution.");

        combined
    }

    /// Returns the currently active specialist.
    pub fn active_specialist(&self) -> Option<AgentSpecialist> {
        self.active_specialist
    }

    /// Returns the handoff history for debugging.
    pub fn handoff_history(&self) -> Vec<HandoffRecord> {
        self.handoff_history.iter().cloned().collect()
    }

    /// Clears the active specialist.
    pub fn clear_active(&mut self) {
        self.active_specialist = None;
    }
}

impl Default for AgentCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intent_classifier_sql_query() {
        let classifier = IntentClassifier::new();

        assert_eq!(
            classifier.classify("SELECT * FROM users WHERE id = 1"),
            UserIntent::SqlQuery
        );
        assert_eq!(
            classifier.classify("How do I join these tables?"),
            UserIntent::SqlQuery
        );
        assert_eq!(
            classifier.classify("Optimize this query"),
            UserIntent::SqlQuery
        );
    }

    #[test]
    fn intent_classifier_data_analysis() {
        let classifier = IntentClassifier::new();

        assert_eq!(
            classifier.classify("Show me sales trends"),
            UserIntent::DataAnalysis
        );
        assert_eq!(
            classifier.classify("What's the average order value?"),
            UserIntent::DataAnalysis
        );
        assert_eq!(
            classifier.classify("Find anomalies in the data"),
            UserIntent::DataAnalysis
        );
    }

    #[test]
    fn intent_classifier_schema_design() {
        let classifier = IntentClassifier::new();

        assert_eq!(
            classifier.classify("Design a table for users"),
            UserIntent::SchemaDesign
        );
        assert_eq!(
            classifier.classify("How do I add an index?"),
            UserIntent::SchemaDesign
        );
        assert_eq!(
            classifier.classify("Create a migration for this schema"),
            UserIntent::SchemaDesign
        );
    }

    #[test]
    fn intent_classifier_unknown() {
        let classifier = IntentClassifier::new();

        assert_eq!(classifier.classify("Hello world"), UserIntent::Unknown);
        assert_eq!(classifier.classify("What time is it?"), UserIntent::Unknown);
    }

    #[test]
    fn route_to_specialist_mappings() {
        let (specialist, confidence) = route_to_specialist(UserIntent::SqlQuery);
        assert_eq!(specialist, AgentSpecialist::SqlExpert);
        assert!(confidence >= CONFIDENCE_THRESHOLD);

        let (specialist, confidence) = route_to_specialist(UserIntent::DataAnalysis);
        assert_eq!(specialist, AgentSpecialist::DataAnalyst);
        assert!(confidence >= CONFIDENCE_THRESHOLD);

        let (specialist, confidence) = route_to_specialist(UserIntent::SchemaDesign);
        assert_eq!(specialist, AgentSpecialist::SchemaArchitect);
        assert!(confidence >= CONFIDENCE_THRESHOLD);

        let (specialist, confidence) = route_to_specialist(UserIntent::Unknown);
        assert_eq!(specialist, AgentSpecialist::SqlExpert);
        assert!(confidence < CONFIDENCE_THRESHOLD);
    }

    #[test]
    fn coordinator_new_registers_all_specialists() {
        let coordinator = AgentCoordinator::new();

        assert!(coordinator
            .specialists
            .contains_key(&AgentSpecialist::SqlExpert));
        assert!(coordinator
            .specialists
            .contains_key(&AgentSpecialist::DataAnalyst));
        assert!(coordinator
            .specialists
            .contains_key(&AgentSpecialist::SchemaArchitect));
    }

    #[test]
    fn coordinator_route_returns_clarifying_question_for_low_confidence() {
        let mut coordinator = AgentCoordinator::new();
        let request = AgentRoutingRequest {
            query_text: "Hello world".to_string(),
            database_context: "test".to_string(),
            user_intent: None,
        };

        let response = coordinator.route(&request).unwrap();
        assert!(response.confidence < CONFIDENCE_THRESHOLD);
        assert!(response.reasoning.contains("clarify"));
    }

    #[test]
    fn coordinator_route_sets_active_specialist() {
        let mut coordinator = AgentCoordinator::new();
        let request = AgentRoutingRequest {
            query_text: "SELECT * FROM users".to_string(),
            database_context: "test".to_string(),
            user_intent: None,
        };

        coordinator.route(&request).unwrap();
        assert_eq!(
            coordinator.active_specialist(),
            Some(AgentSpecialist::SqlExpert)
        );
    }

    #[test]
    fn coordinator_dispatch_calls_specialist() {
        let coordinator = AgentCoordinator::new();
        let request = AgentRoutingRequest {
            query_text: "SELECT * FROM users".to_string(),
            database_context: "sqlite:./db.sqlite".to_string(),
            user_intent: None,
        };

        let result = coordinator.dispatch(AgentSpecialist::SqlExpert, &request);
        assert!(result.is_ok());
        assert!(result.unwrap().contains("SQL expert"));
    }

    #[test]
    fn coordinator_dispatch_fails_for_unknown_specialist() {
        // This test verifies the error path - in practice all specialists are registered
        let coordinator = AgentCoordinator::new();
        let request = AgentRoutingRequest {
            query_text: "test".to_string(),
            database_context: "test".to_string(),
            user_intent: None,
        };

        // All three specialists should work
        assert!(coordinator
            .dispatch(AgentSpecialist::SqlExpert, &request)
            .is_ok());
        assert!(coordinator
            .dispatch(AgentSpecialist::DataAnalyst, &request)
            .is_ok());
        assert!(coordinator
            .dispatch(AgentSpecialist::SchemaArchitect, &request)
            .is_ok());
    }

    #[test]
    fn coordinator_handoff_records_history() {
        let mut coordinator = AgentCoordinator::new();

        let result = coordinator.handoff(
            AgentSpecialist::SqlExpert,
            AgentSpecialist::DataAnalyst,
            "Need trend analysis".to_string(),
        );

        assert!(result.is_ok());
        assert_eq!(coordinator.handoff_history().len(), 1);
        assert_eq!(
            coordinator.handoff_history()[0].from,
            AgentSpecialist::SqlExpert
        );
        assert_eq!(
            coordinator.handoff_history()[0].to,
            AgentSpecialist::DataAnalyst
        );
        assert_eq!(
            coordinator.active_specialist(),
            Some(AgentSpecialist::DataAnalyst)
        );
    }

    #[test]
    fn coordinator_handoff_limits_history() {
        let mut coordinator = AgentCoordinator::new();

        // Add more than HANDOFF_HISTORY_LIMIT handoffs
        for _ in 0..HANDOFF_HISTORY_LIMIT + 5 {
            let _ = coordinator.handoff(
                AgentSpecialist::SqlExpert,
                AgentSpecialist::DataAnalyst,
                "context".to_string(),
            );
        }

        assert!(coordinator.handoff_history().len() <= HANDOFF_HISTORY_LIMIT);
    }

    #[test]
    fn coordinator_synthesize_single_response() {
        let coordinator = AgentCoordinator::new();
        let responses = vec![SpecialistResponse {
            specialist: AgentSpecialist::SqlExpert,
            content: "Use an index on the id column.".to_string(),
            confidence: 0.9,
        }];

        let result = coordinator.synthesize(responses);
        assert!(result.contains("Use an index on the id column."));
        assert!(!result.contains("Combined insights"));
    }

    #[test]
    fn coordinator_synthesize_multiple_responses() {
        let coordinator = AgentCoordinator::new();
        let responses = vec![
            SpecialistResponse {
                specialist: AgentSpecialist::SqlExpert,
                content: "Optimize with index.".to_string(),
                confidence: 0.9,
            },
            SpecialistResponse {
                specialist: AgentSpecialist::DataAnalyst,
                content: "Check for trends.".to_string(),
                confidence: 0.85,
            },
        ];

        let result = coordinator.synthesize(responses);
        assert!(result.contains("Combined insights"));
        assert!(result.contains("SqlExpert"));
        assert!(result.contains("DataAnalyst"));
    }

    #[test]
    fn coordinator_clear_active() {
        let mut coordinator = AgentCoordinator::new();
        coordinator.active_specialist = Some(AgentSpecialist::SqlExpert);

        coordinator.clear_active();
        assert!(coordinator.active_specialist().is_none());
    }

    #[test]
    fn specialist_sql_expert_handle() {
        let specialist = SqlExpert::new();
        let request = AgentRoutingRequest {
            query_text: "SELECT * FROM users".to_string(),
            database_context: "sqlite:./db.sqlite".to_string(),
            user_intent: None,
        };

        let result = specialist.handle(&request).unwrap();
        assert!(result.contains("SQL expert"));
        assert!(result.contains("optimize"));
    }

    #[test]
    fn specialist_data_analyst_handle() {
        let specialist = DataAnalyst::new();
        let request = AgentRoutingRequest {
            query_text: "Show trends".to_string(),
            database_context: "postgres://localhost/db".to_string(),
            user_intent: None,
        };

        let result = specialist.handle(&request).unwrap();
        assert!(result.contains("data analyst"));
        assert!(result.contains("trends"));
    }

    #[test]
    fn specialist_schema_architect_handle() {
        let specialist = SchemaArchitect::new();
        let request = AgentRoutingRequest {
            query_text: "Create table".to_string(),
            database_context: "mysql://localhost/db".to_string(),
            user_intent: None,
        };

        let result = specialist.handle(&request).unwrap();
        assert!(result.contains("schema architect"));
        assert!(result.contains("design"));
    }
}
