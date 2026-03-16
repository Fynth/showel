use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq)]
pub struct TablePreviewSource {
    pub schema: Option<String>,
    pub table_name: String,
    pub qualified_name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EditableTableContext {
    pub source: TablePreviewSource,
    pub row_locators: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct QueryPage {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub editable: Option<EditableTableContext>,
    pub offset: u64,
    pub page_size: u32,
    pub has_previous: bool,
    pub has_next: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum QueryOutput {
    Table(QueryPage),
    AffectedRows(u64),
}

#[derive(Clone, Debug)]
pub struct QueryTabState {
    pub id: u64,
    pub session_id: u64,
    pub title: String,
    pub sql: String,
    pub status: String,
    pub result: Option<QueryOutput>,
    pub current_offset: u64,
    pub page_size: u32,
    pub last_run_sql: Option<String>,
    pub preview_source: Option<TablePreviewSource>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryHistoryItem {
    pub id: u64,
    pub tab_title: String,
    #[serde(default)]
    pub connection_name: String,
    pub sql: String,
    pub outcome: String,
}
