use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatThreadSummary {
    pub id: i64,
    pub title: String,
    pub connection_name: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_message_preview: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChatArtifact {
    SqlDraft { sql: String },
    QuerySummary { sql: String, summary: String },
}
