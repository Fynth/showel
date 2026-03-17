#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExplorerNodeKind {
    Schema,
    Table,
    View,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExplorerNode {
    pub name: String,
    pub kind: ExplorerNodeKind,
    pub schema: Option<String>,
    pub qualified_name: String,
    pub children: Vec<ExplorerNode>,
}
