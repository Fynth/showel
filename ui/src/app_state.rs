use dioxus::prelude::*;
use models::{
    AppState, AppThemePreference, AppUiSettings, ConnectionRequest, ConnectionSession,
    DatabaseConnection, SqlFormatSettings,
};

#[derive(Clone, Debug, PartialEq)]
pub struct AppTooltip {
    pub label: String,
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AppToast {
    pub id: u64,
    pub message: String,
    pub kind: ToastKind,
}

#[derive(Clone, Debug, PartialEq)]
#[allow(dead_code)]
pub enum ToastKind {
    Info,
    Success,
    Warning,
    Error,
}

pub static APP_STATE: GlobalSignal<AppState> = Signal::global(AppState::default);
pub static APP_THEME: GlobalSignal<String> =
    Signal::global(|| AppThemePreference::Dark.css_class().to_string());
pub static APP_UI_SETTINGS: GlobalSignal<AppUiSettings> = Signal::global(AppUiSettings::default);
pub static APP_SQL_FORMAT_SETTINGS: GlobalSignal<SqlFormatSettings> =
    Signal::global(SqlFormatSettings::default);
pub static APP_SHOW_HISTORY: GlobalSignal<bool> = Signal::global(|| false);
pub static APP_SHOW_SETTINGS_MODAL: GlobalSignal<bool> = Signal::global(|| false);
pub static APP_TOOLTIP: GlobalSignal<Option<AppTooltip>> = Signal::global(|| None);
pub static APP_TOAST: GlobalSignal<Vec<AppToast>> = Signal::global(Vec::new);
static NEXT_TOAST_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

pub fn open_settings_modal() {
    *APP_SHOW_SETTINGS_MODAL.write() = true;
}

pub fn close_settings_modal() {
    *APP_SHOW_SETTINGS_MODAL.write() = false;
}

pub fn show_tooltip(label: String, x: f64, y: f64) {
    *APP_TOOLTIP.write() = Some(AppTooltip { label, x, y });
}

pub fn hide_tooltip() {
    *APP_TOOLTIP.write() = None;
}

pub fn show_toast(message: impl Into<String>, kind: ToastKind) {
    let id = NEXT_TOAST_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let toast = AppToast {
        id,
        message: message.into(),
        kind,
    };
    APP_TOAST.with_mut(|toasts| {
        toasts.push(toast);
    });
    let toast_id = id;
    spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        dismiss_toast(toast_id);
    });
}

pub fn dismiss_toast(id: u64) {
    APP_TOAST.with_mut(|toasts| {
        toasts.retain(|t| t.id != id);
    });
}

pub fn toast_error(message: impl Into<String>) {
    show_toast(message, ToastKind::Error);
}

#[allow(dead_code)]
pub fn toast_info(message: impl Into<String>) {
    show_toast(message, ToastKind::Info);
}

#[allow(dead_code)]
pub fn toast_success(message: impl Into<String>) {
    show_toast(message, ToastKind::Success);
}

#[allow(dead_code)]
pub fn toast_warning(message: impl Into<String>) {
    show_toast(message, ToastKind::Warning);
}

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
    APP_STATE.read().session_connection(session_id).cloned()
}

pub fn add_connection_session(request: ConnectionRequest, connection: DatabaseConnection) -> u64 {
    let session_name = request.display_name();
    let session_kind = request.kind();
    let session_key = request.identity_key();

    let mut activated_id = 0;
    APP_STATE.with_mut(|state| {
        if let Some(existing_session) = state
            .sessions
            .iter_mut()
            .find(|session| session.request.identity_key() == session_key)
        {
            existing_session.request = request.clone();
            existing_session.connection = connection.clone();
            existing_session.name = session_name.clone();
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
        let removed_keys = state
            .sessions
            .iter()
            .filter(|session| session.id == session_id)
            .map(|session| session.request.identity_key())
            .collect::<Vec<_>>();

        state.sessions.retain(|session| session.id != session_id);

        if state.active_session_id == Some(session_id) {
            state.active_session_id = state.sessions.first().map(|session| session.id);
        }

        if state.sessions.is_empty() {
            state.active_session_id = None;
            state.show_connection_screen = true;
        }

        for key in removed_keys {
            connection::release_ssh_tunnel(&key);
        }
    });
    persist_session_state();
}

pub fn restore_connection_sessions(
    restored: Vec<(ConnectionRequest, DatabaseConnection)>,
    active_name: Option<String>,
) {
    // First collect existing session names and release SSH tunnels
    let existing_keys = {
        let state = APP_STATE.read();
        state
            .sessions
            .iter()
            .map(|session| session.request.identity_key())
            .collect::<Vec<_>>()
    };

    // Release SSH tunnels outside the lock to avoid potential deadlocks
    for key in existing_keys {
        connection::release_ssh_tunnel(&key);
    }

    // Now replace sessions atomically
    APP_STATE.with_mut(|state| {
        let mut new_sessions = Vec::with_capacity(restored.len());
        let mut next_id = 1;

        for (request, connection) in restored {
            let session_name = request.display_name();
            let session_kind = request.kind();
            new_sessions.push(ConnectionSession {
                id: next_id,
                name: session_name,
                kind: session_kind,
                request,
                connection,
            });
            next_id += 1;
        }

        state.sessions = new_sessions;
        state.next_session_id = next_id;
        state.active_session_id = active_name
            .as_deref()
            .and_then(|active_name| {
                state
                    .sessions
                    .iter()
                    .find(|session| {
                        session.request.identity_key() == active_name || session.name == active_name
                    })
                    .map(|session| session.id)
            })
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
            .and_then(|active_id| state.session(active_id))
            .map(|session| session.request.identity_key());
        (requests, active)
    };

    // Use spawn_blocking to avoid blocking the UI thread
    tokio::task::spawn_blocking(move || {
        if let Err(err) = storage::save_session_state_sync(open_requests, active_connection_name) {
            eprintln!("Failed to persist session state: {}", err);
        }
    });
}
