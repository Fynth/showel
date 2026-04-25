use dioxus::prelude::*;
use models::{ChatThreadSummary, QueryHistoryItem, SavedQuery};

use crate::app_state::{APP_AI_FEATURES_ENABLED, toast_error};

#[allow(dead_code)]
pub struct ChatState {
    pub chat_threads: Signal<Vec<ChatThreadSummary>>,
    pub active_chat_thread_id: Signal<Option<i64>>,
    pub chat_revision: Signal<u64>,
    pub chat_threads_loaded: Signal<bool>,
    pub chat_bootstrap_inflight: Signal<bool>,
    pub history: Signal<Vec<QueryHistoryItem>>,
    pub next_history_id: Signal<u64>,
    pub saved_queries: Signal<Vec<SavedQuery>>,
    pub next_saved_query_id: Signal<u64>,
}

pub fn use_chat_state(connection_label: String) -> ChatState {
    let mut chat_threads = use_signal(Vec::<ChatThreadSummary>::new);
    let mut active_chat_thread_id = use_signal(|| None::<i64>);
    let chat_revision = use_signal(|| 0_u64);
    let mut chat_threads_loaded = use_signal(|| false);
    let mut chat_bootstrap_inflight = use_signal(|| false);

    let mut history = use_signal(Vec::<QueryHistoryItem>::new);
    let mut next_history_id = use_signal(|| 1_u64);
    let mut saved_queries = use_signal(Vec::<SavedQuery>::new);
    let mut next_saved_query_id = use_signal(|| 1_u64);

    // ── Persisted data caches ──────────────────────────────────────
    static HISTORY_CACHE: std::sync::LazyLock<
        std::sync::Mutex<Option<Vec<models::QueryHistoryItem>>>,
    > = std::sync::LazyLock::new(|| std::sync::Mutex::new(None));

    static SAVED_QUERIES_CACHE: std::sync::LazyLock<
        std::sync::Mutex<Option<Vec<models::SavedQuery>>>,
    > = std::sync::LazyLock::new(|| std::sync::Mutex::new(None));

    let persisted_history = use_resource(move || async move {
        let cached = { HISTORY_CACHE.lock().ok().and_then(|cache| cache.clone()) };
        if let Some(data) = cached {
            return data;
        }

        let data = services::load_query_history().await.unwrap_or_default();
        if let Ok(mut cache) = HISTORY_CACHE.lock() {
            *cache = Some(data.clone());
        }
        data
    });

    let persisted_saved_queries = use_resource(move || async move {
        let cached = {
            SAVED_QUERIES_CACHE
                .lock()
                .ok()
                .and_then(|cache| cache.clone())
        };
        if let Some(data) = cached {
            return data;
        }

        let data = services::load_saved_queries().await.unwrap_or_default();
        if let Ok(mut cache) = SAVED_QUERIES_CACHE.lock() {
            *cache = Some(data.clone());
        }
        data
    });

    // ── Effect: populate history from persisted data ───────────────
    use_effect(move || {
        if let Some(items) = persisted_history() {
            let next_id = items.iter().map(|item| item.id).max().unwrap_or(0) + 1;
            history.set(items);
            next_history_id.set(next_id);
        }
    });

    // ── Effect: populate saved queries from persisted data ─────────
    use_effect(move || {
        if let Some(items) = persisted_saved_queries() {
            let next_id = items.iter().map(|item| item.id).max().unwrap_or(0) + 1;
            saved_queries.set(items);
            next_saved_query_id.set(next_id);
        }
    });

    // ── Effect: bootstrap chat threads ─────────────────────────────
    let connection_label_for_bootstrap = connection_label.clone();

    use_effect(move || {
        if !APP_AI_FEATURES_ENABLED() {
            return;
        }
        if chat_threads_loaded() {
            return;
        }
        if chat_bootstrap_inflight() {
            return;
        }

        chat_bootstrap_inflight.set(true);
        let default_connection = connection_label_for_bootstrap.clone();
        spawn(async move {
            let items = services::load_chat_threads().await.unwrap_or_default();
            if items.is_empty() {
                match services::create_chat_thread(default_connection, Some("New chat".to_string()))
                    .await
                {
                    Ok(thread) => {
                        chat_threads.set(vec![thread.clone()]);
                        active_chat_thread_id.set(Some(thread.id));
                    }
                    Err(err) => {
                        toast_error(format!("Failed to create default chat thread: {err}"));
                    }
                }
            } else {
                let next_active_thread_id = active_chat_thread_id()
                    .filter(|thread_id| items.iter().any(|thread| thread.id == *thread_id))
                    .or_else(|| items.first().map(|thread| thread.id));
                chat_threads.set(items);
                active_chat_thread_id.set(next_active_thread_id);
            }

            chat_threads_loaded.set(true);
            chat_bootstrap_inflight.set(false);
        });
    });

    ChatState {
        chat_threads,
        active_chat_thread_id,
        chat_revision,
        chat_threads_loaded,
        chat_bootstrap_inflight,
        history,
        next_history_id,
        saved_queries,
        next_saved_query_id,
    }
}
