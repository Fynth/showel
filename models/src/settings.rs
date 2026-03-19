use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WorkspaceToolPanel {
    Connections,
    Explorer,
    SavedQueries,
    History,
    Agent,
}

impl WorkspaceToolPanel {
    pub const ALL: [Self; 5] = [
        Self::Connections,
        Self::Explorer,
        Self::SavedQueries,
        Self::History,
        Self::Agent,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Connections => "Connections",
            Self::Explorer => "Explorer",
            Self::SavedQueries => "Saved Queries",
            Self::History => "History",
            Self::Agent => "ACP Agent",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkspaceToolDock {
    Sidebar,
    Inspector,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkspaceToolLayout {
    pub sidebar: Vec<WorkspaceToolPanel>,
    pub inspector: Vec<WorkspaceToolPanel>,
}

impl WorkspaceToolLayout {
    pub fn normalized(&self) -> Self {
        let defaults = Self::default();
        let mut sidebar = Vec::with_capacity(WorkspaceToolPanel::ALL.len());
        let mut inspector = Vec::with_capacity(WorkspaceToolPanel::ALL.len());
        let mut seen = Vec::with_capacity(WorkspaceToolPanel::ALL.len());

        let mut push_unique = |items: &[WorkspaceToolPanel],
                               target: &mut Vec<WorkspaceToolPanel>| {
            for panel in items {
                if seen.contains(panel) {
                    continue;
                }
                seen.push(*panel);
                target.push(*panel);
            }
        };

        push_unique(&self.sidebar, &mut sidebar);
        push_unique(&self.inspector, &mut inspector);
        push_unique(&defaults.sidebar, &mut sidebar);
        push_unique(&defaults.inspector, &mut inspector);

        Self { sidebar, inspector }
    }

    pub fn dock_for(&self, panel: WorkspaceToolPanel) -> WorkspaceToolDock {
        if self.inspector.contains(&panel) {
            WorkspaceToolDock::Inspector
        } else {
            WorkspaceToolDock::Sidebar
        }
    }
}

impl Default for WorkspaceToolLayout {
    fn default() -> Self {
        Self {
            sidebar: vec![
                WorkspaceToolPanel::Connections,
                WorkspaceToolPanel::Explorer,
                WorkspaceToolPanel::SavedQueries,
                WorkspaceToolPanel::History,
            ],
            inspector: vec![WorkspaceToolPanel::Agent],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppThemePreference {
    Dark,
    Light,
}

impl AppThemePreference {
    pub fn css_class(self) -> &'static str {
        match self {
            Self::Dark => "theme-dark",
            Self::Light => "theme-light",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Dark => "Dark",
            Self::Light => "Light",
        }
    }
}

impl Default for AppThemePreference {
    fn default() -> Self {
        Self::Dark
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppUiSettings {
    pub theme: AppThemePreference,
    pub restore_session_on_launch: bool,
    pub show_connections: bool,
    pub show_explorer: bool,
    pub show_history: bool,
    pub show_sql_editor: bool,
    pub show_agent_panel: bool,
    pub default_page_size: u32,
    pub tool_panel_layout: WorkspaceToolLayout,
}

impl Default for AppUiSettings {
    fn default() -> Self {
        Self {
            theme: AppThemePreference::Dark,
            restore_session_on_launch: true,
            show_connections: false,
            show_explorer: true,
            show_history: false,
            show_sql_editor: true,
            show_agent_panel: false,
            default_page_size: 100,
            tool_panel_layout: WorkspaceToolLayout::default(),
        }
    }
}
