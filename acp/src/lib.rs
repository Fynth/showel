pub mod agents;
pub mod context;
#[cfg(feature = "embedding")]
pub mod embedding;
pub mod introspection;
pub mod ollama;
pub mod runtime;
#[cfg(feature = "embedding")]
pub mod semantic_cache;

pub use acp_registry::{install_acp_registry_agent, load_acp_registry_agents};
pub use agents::{
    AgentCoordinator, DataAnalyst, HandoffRecord, IntentClassifier, SchemaArchitect, Specialist,
    SpecialistResponse, SqlExpert, UserIntent,
};
pub use context::{build_acp_database_context, warm_acp_database_schema_context};
#[cfg(feature = "embedding")]
pub use embedding::{
    EMBEDDING_DIM, EmbeddingModel, LazyEmbeddingModel, MODEL_FILENAME, cosine_similarity,
};
pub use introspection::{
    ActiveQueryInfo, ColumnInfo, IndexInfo, IndexStat, IntrospectionConfig, IntrospectionPool,
    IntrospectionRateLimiter, IntrospectionResult, LockInfo, QueryHistoryEntry, SchemaInfo,
    TableInfo, TableStat, explain_query_plan_mysql, explain_query_plan_postgres,
    explain_query_plan_sqlite,
};
pub use ollama::{
    EmbeddedOllamaAgentConfig, OllamaSpecialistAdapter, build_embedded_ollama_launch,
    load_ollama_models, run_embedded_ollama_agent,
};
pub use runtime::{
    cancel_acp_prompt, clear_active_specialist, connect_acp_agent, disconnect_acp_agent,
    drain_acp_events, get_active_specialist, record_execution, respond_acp_permission,
    route_acp_request, send_acp_prompt, send_acp_prompt_with_routing,
};
#[cfg(feature = "embedding")]
pub use semantic_cache::{SemanticCache, SemanticCacheBuilder};
