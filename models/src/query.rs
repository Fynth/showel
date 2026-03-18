use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SqlKeywordCase {
    Preserve,
    Uppercase,
    Lowercase,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SqlFormatSettings {
    pub keyword_case: SqlKeywordCase,
    pub indent_width: u8,
    pub lines_between_queries: u8,
    pub inline: bool,
    pub joins_as_top_level: bool,
    pub max_inline_block: u8,
    pub max_inline_arguments: Option<u8>,
    pub max_inline_top_level: Option<u8>,
}

impl Default for SqlFormatSettings {
    fn default() -> Self {
        Self {
            keyword_case: SqlKeywordCase::Uppercase,
            indent_width: 2,
            lines_between_queries: 1,
            inline: false,
            joins_as_top_level: true,
            max_inline_block: 40,
            max_inline_arguments: Some(4),
            max_inline_top_level: Some(40),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TablePreviewSource {
    pub schema: Option<String>,
    pub table_name: String,
    pub qualified_name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuerySort {
    pub column_name: String,
    pub descending: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QueryFilterMode {
    And,
    Or,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueryFilterOperator {
    Contains,
    NotContains,
    Equals,
    NotEquals,
    StartsWith,
    EndsWith,
    IsNull,
    IsNotNull,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueryFilterRule {
    pub column_name: String,
    pub operator: QueryFilterOperator,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueryFilter {
    pub mode: QueryFilterMode,
    pub rules: Vec<QueryFilterRule>,
}

impl QueryFilterOperator {
    pub fn is_nullary(self) -> bool {
        matches!(self, Self::IsNull | Self::IsNotNull)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EditableTableContext {
    pub source: TablePreviewSource,
    pub row_locators: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct PendingTableChanges {
    pub next_insert_id: u64,
    pub inserted_rows: Vec<PendingInsertRow>,
    pub updated_cells: Vec<PendingCellChange>,
}

impl PendingTableChanges {
    pub fn is_empty(&self) -> bool {
        self.inserted_rows.is_empty() && self.updated_cells.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingInsertRow {
    pub id: u64,
    pub values: Vec<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingCellChange {
    pub locator: String,
    pub column_name: String,
    pub value: String,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkspaceTabKind {
    Query,
    TablePreview,
    Structure,
}

#[derive(Clone, Debug, PartialEq)]
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
    pub filter: Option<QueryFilter>,
    pub sort: Option<QuerySort>,
    pub tab_kind: WorkspaceTabKind,
    pub is_loading_more: bool,
    pub pending_table_changes: PendingTableChanges,
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
