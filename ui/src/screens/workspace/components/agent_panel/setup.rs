use dioxus::prelude::*;
use models::{AcpMessageKind, AcpPanelState};

use super::messages::acp_registry_preparing_text;
use super::state::{apply_connected, push_message};

const OPENCODE_REGISTRY_AGENT_ID: &str = "opencode";
const CODEX_REGISTRY_AGENT_ID: &str = "codex-acp";

async fn connect_registry_agent(
    mut panel_state: Signal<AcpPanelState>,
    mut chat_revision: Signal<u64>,
    agent_id: &str,
    agent_name: &str,
) -> Result<(), String> {
    let cwd = panel_state().launch.cwd.clone();
    panel_state.with_mut(|state| {
        state.busy = true;
        state.status = acp_registry_preparing_text(agent_name);
    });

    let launch = match acp::install_acp_registry_agent(agent_id.to_string(), cwd).await {
        Ok(launch) => launch,
        Err(err) => {
            panel_state.with_mut(|state| {
                state.busy = false;
                state.status = err.clone();
                push_message(state, AcpMessageKind::Error, err.clone());
            });
            chat_revision += 1;
            return Err(err);
        }
    };

    panel_state.with_mut(|state| {
        state.launch = launch.clone();
        state.busy = true;
        state.status = format!("Connecting to {agent_name}...");
    });

    match acp::connect_acp_agent(launch).await {
        Ok(connection) => {
            panel_state.with_mut(|state| {
                apply_connected(state, connection);
            });
            Ok(())
        }
        Err(err) => {
            panel_state.with_mut(|state| {
                state.busy = false;
                state.connected = false;
                state.connection = None;
                state.status = err.clone();
                push_message(state, AcpMessageKind::Error, err.clone());
            });
            chat_revision += 1;
            Err(err)
        }
    }
}

pub(crate) async fn ensure_opencode_connected(
    panel_state: Signal<AcpPanelState>,
    chat_revision: Signal<u64>,
) -> Result<(), String> {
    if panel_state().connected {
        return Ok(());
    }

    if panel_state().busy {
        let status = panel_state().status.trim().to_string();
        return Err(if status.is_empty() {
            "ACP agent is busy.".to_string()
        } else {
            status
        });
    }

    connect_registry_agent(
        panel_state,
        chat_revision,
        OPENCODE_REGISTRY_AGENT_ID,
        "OpenCode",
    )
    .await
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum AgentSetupMode {
    Ollama,
    OpenCode,
    Codex,
    Custom,
}

impl AgentSetupMode {
    pub(super) const ALL: [Self; 4] = [Self::Ollama, Self::OpenCode, Self::Codex, Self::Custom];

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Ollama => "Ollama",
            Self::OpenCode => "OpenCode",
            Self::Codex => "Codex",
            Self::Custom => "Custom",
        }
    }

    pub(super) fn meta(self) -> &'static str {
        match self {
            Self::Ollama => "Embedded",
            Self::OpenCode | Self::Codex => "Registry",
            Self::Custom => "stdio",
        }
    }

    pub(super) fn registry_agent_id(self) -> Option<&'static str> {
        match self {
            Self::OpenCode => Some(OPENCODE_REGISTRY_AGENT_ID),
            Self::Codex => Some(CODEX_REGISTRY_AGENT_ID),
            Self::Ollama | Self::Custom => None,
        }
    }

    pub(super) fn registry_name(self) -> Option<&'static str> {
        match self {
            Self::OpenCode => Some("OpenCode"),
            Self::Codex => Some("Codex CLI"),
            Self::Ollama | Self::Custom => None,
        }
    }

    pub(super) fn registry_hint(self) -> Option<&'static str> {
        match self {
            Self::OpenCode => Some("OpenCode agent."),
            Self::Codex => Some("Codex CLI agent."),
            Self::Ollama | Self::Custom => None,
        }
    }
}

pub(super) fn setup_mode_button_class(
    mode: AgentSetupMode,
    active_mode: AgentSetupMode,
) -> &'static str {
    if mode == active_mode {
        "button button--ghost button--active agent-panel__mode-button"
    } else {
        "button button--ghost agent-panel__mode-button"
    }
}
