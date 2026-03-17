use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SavedQueryKind {
    Query,
    Snippet,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedQuery {
    pub id: u64,
    pub title: String,
    #[serde(default)]
    pub folder: String,
    pub sql: String,
    pub kind: SavedQueryKind,
    #[serde(default)]
    pub connection_name: Option<String>,
}

impl SavedQuery {
    pub fn folder_name(&self) -> &str {
        let trimmed = self.folder.trim();
        if trimmed.is_empty() {
            "General"
        } else {
            trimmed
        }
    }

    pub fn kind_label(&self) -> &'static str {
        match self.kind {
            SavedQueryKind::Query => "Query",
            SavedQueryKind::Snippet => "Snippet",
        }
    }
}
