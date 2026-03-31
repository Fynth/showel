pub mod agents;
pub mod context;
pub mod embedding;
pub mod introspection;
pub mod ollama;
pub mod runtime;
pub mod semantic_cache;

pub use acp_registry::{install_acp_registry_agent, load_acp_registry_agents};
pub use agents::{
    AgentCoordinator, DataAnalyst, HandoffRecord, IntentClassifier, SchemaArchitect, Specialist,
    SpecialistResponse, SqlExpert, UserIntent,
};
pub use context::{build_acp_database_context, warm_acp_database_schema_context};
pub use embedding::{cosine_similarity, EmbeddingModel, EMBEDDING_DIM, MODEL_FILENAME};
pub use introspection::{
    explain_query_plan_mysql, explain_query_plan_postgres, explain_query_plan_sqlite,
    ActiveQueryInfo, ColumnInfo, IndexInfo, IndexStat, IntrospectionConfig, IntrospectionPool,
    IntrospectionRateLimiter, IntrospectionResult, LockInfo, QueryHistoryEntry, SchemaInfo,
    TableInfo, TableStat,
};
pub use ollama::{
    EmbeddedOllamaAgentConfig, build_embedded_ollama_launch, load_ollama_models,
    run_embedded_ollama_agent,
};
pub use runtime::{
    cancel_acp_prompt, connect_acp_agent, disconnect_acp_agent, drain_acp_events,
    respond_acp_permission, send_acp_prompt,
};
pub use semantic_cache::{SemanticCache, SemanticCacheBuilder};
