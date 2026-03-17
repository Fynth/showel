use dioxus::prelude::*;
use models::{AppState, ConnectionRequest, ConnectionSession, DatabaseConnection};

pub static APP_STATE: GlobalSignal<AppState> = Signal::global(AppState::default);
pub static APP_THEME: GlobalSignal<String> = Signal::global(|| "theme-dark".to_string());
pub static APP_SHOW_HISTORY: GlobalSignal<bool> = Signal::global(|| false);

pub fn open_connection_screen() {
    APP_STATE.with_mut(|state| {
        state.show_connection_screen = true;
    });
}

pub fn show_workspace() {
    APP_STATE.with_mut(|state| {
        state.show_connection_screen = false;
    });
}

pub fn activate_session(session_id: u64) {
    APP_STATE.with_mut(|state| {
        if state
            .sessions
            .iter()
            .any(|session| session.id == session_id)
        {
            state.active_session_id = Some(session_id);
            state.show_connection_screen = false;
        }
    });
    persist_session_state();
}

pub fn session_connection(session_id: u64) -> Option<DatabaseConnection> {
    APP_STATE.read().session_connection(session_id)
}

pub fn add_connection_session(request: ConnectionRequest, connection: DatabaseConnection) -> u64 {
    let session_name = request.display_name();
    let session_kind = request.kind();

    let mut activated_id = 0;
    APP_STATE.with_mut(|state| {
        if let Some(existing_session) = state
            .sessions
            .iter_mut()
            .find(|session| session.name == session_name)
        {
            existing_session.request = request.clone();
            existing_session.connection = connection.clone();
            existing_session.kind = session_kind;
            activated_id = existing_session.id;
        } else {
            let session_id = state.next_session_id;
            state.next_session_id += 1;
            state.sessions.push(ConnectionSession {
                id: session_id,
                name: session_name,
                kind: session_kind,
                request,
                connection,
            });
            activated_id = session_id;
        }

        state.active_session_id = Some(activated_id);
        state.show_connection_screen = false;
    });

    persist_session_state();

    activated_id
}

pub fn remove_session(session_id: u64) {
    APP_STATE.with_mut(|state| {
        state.sessions.retain(|session| session.id != session_id);

        if state.active_session_id == Some(session_id) {
            state.active_session_id = state.sessions.first().map(|session| session.id);
        }

        if state.sessions.is_empty() {
            state.active_session_id = None;
            state.show_connection_screen = true;
        }
    });
    persist_session_state();
}

pub fn restore_connection_sessions(
    restored: Vec<(ConnectionRequest, DatabaseConnection)>,
    active_name: Option<String>,
) {
    APP_STATE.with_mut(|state| {
        state.sessions.clear();
        state.active_session_id = None;
        state.next_session_id = 1;

        for (request, connection) in restored {
            let session_id = state.next_session_id;
            state.next_session_id += 1;
            let session_name = request.display_name();
            let session_kind = request.kind();
            state.sessions.push(ConnectionSession {
                id: session_id,
                name: session_name,
                kind: session_kind,
                request,
                connection,
            });
        }

        state.active_session_id = active_name
            .as_deref()
            .and_then(|name| state.session_id_by_name(name))
            .or_else(|| state.sessions.first().map(|session| session.id));
        state.show_connection_screen = state.sessions.is_empty();
    });
    persist_session_state();
}

fn persist_session_state() {
    let (open_requests, active_connection_name) = {
        let state = APP_STATE.read();
        let requests = state
            .sessions
            .iter()
            .map(|session| session.request.clone())
            .collect::<Vec<_>>();
        let active = state
            .active_session_id
            .and_then(|active_id| state.session_name(active_id));
        (requests, active)
    };

    let _ = services::save_session_state_sync(open_requests, active_connection_name);
}
