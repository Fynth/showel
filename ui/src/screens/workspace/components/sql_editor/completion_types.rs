#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompletionKind {
    Table,
    View,
    Column,
    Keyword,
    Function,
    Schema,
    Alias,
}

impl CompletionKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Table => "T",
            Self::View => "V",
            Self::Column => "C",
            Self::Keyword => "K",
            Self::Function => "F",
            Self::Schema => "S",
            Self::Alias => "A",
        }
    }
}

#[derive(Clone, Debug)]
pub struct CompletionItem {
    pub label: String,
    pub kind: CompletionKind,
    pub detail: Option<String>,
    pub insert_text: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TableMeta {
    pub schema: Option<String>,
    pub name: String,
    pub qualified_name: String,
    pub columns: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SchemaMetadata {
    pub tables: Vec<TableMeta>,
    pub schemas: Vec<String>,
}
