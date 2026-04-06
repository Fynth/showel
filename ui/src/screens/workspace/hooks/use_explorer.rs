use dioxus::prelude::*;

use super::super::components::ExplorerConnectionSection;
use super::super::helpers::{load_explorer_section, unloaded_explorer_section};
use crate::app_state::APP_STATE;

pub struct ExplorerState {
    pub tree_status: Signal<String>,
    pub tree_sections: Signal<Vec<ExplorerConnectionSection>>,
    pub tree_reload: Signal<u64>,
}

pub fn use_explorer_state(show_explorer: Signal<bool>) -> ExplorerState {
    let mut tree_status = use_signal(|| "Loading explorer...".to_string());
    let mut tree_sections = use_signal(Vec::<ExplorerConnectionSection>::new);
    let tree_reload = use_signal(|| 0_u64);

    use_effect(move || {
        let reload_tick = tree_reload();
        let explorer_visible = show_explorer();
        let (sessions, active_session_id) = {
            let app_state = APP_STATE.read();
            (app_state.sessions.clone(), app_state.active_session_id)
        };

        spawn(async move {
            let _ = reload_tick;
            if sessions.is_empty() {
                tree_sections.set(Vec::new());
                tree_status.set("Select or create a connection".to_string());
                return;
            }

            if !explorer_visible {
                tree_sections.set(
                    sessions
                        .iter()
                        .map(|session| {
                            unloaded_explorer_section(session, active_session_id, "Explorer hidden")
                        })
                        .collect(),
                );
                tree_status.set("Explorer hidden".to_string());
                return;
            }

            let active_index = sessions
                .iter()
                .position(|session| Some(session.id) == active_session_id)
                .unwrap_or(0);
            let mut sections = sessions
                .iter()
                .map(|session| {
                    unloaded_explorer_section(
                        session,
                        active_session_id,
                        "Activate this connection to load explorer",
                    )
                })
                .collect::<Vec<_>>();

            tree_status.set("Loading explorer...".to_string());
            let active_section = load_explorer_section(
                sessions[active_index].clone(),
                active_session_id.or(Some(sessions[active_index].id)),
            )
            .await;
            let active_failed = active_section.status.starts_with("Error:");
            sections[active_index] = active_section;

            tree_sections.set(sections);
            if active_failed {
                tree_status.set("Explorer failed for the active connection".to_string());
            } else {
                tree_status.set("Explorer ready for the active connection".to_string());
            }
        });
    });

    ExplorerState {
        tree_status,
        tree_sections,
        tree_reload,
    }
}
