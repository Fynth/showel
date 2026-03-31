pub mod context;
pub mod embedding;
pub mod ollama;
pub mod runtime;

pub use acp_registry::{install_acp_registry_agent, load_acp_registry_agents};
pub use context::{build_acp_database_context, warm_acp_database_schema_context};
pub use embedding::{EmbeddingModel, MODEL_FILENAME};
pub use ollama::{
    EmbeddedOllamaAgentConfig, build_embedded_ollama_launch, load_ollama_models,
    run_embedded_ollama_agent,
};
pub use runtime::{
    cancel_acp_prompt, connect_acp_agent, disconnect_acp_agent, drain_acp_events,
    respond_acp_permission, send_acp_prompt,
};
