use dioxus::prelude::*;
use models::ChatThreadSummary;

use super::helpers::upsert_chat_thread_summary;
use crate::app_state::toast_error;

pub fn create_chat_thread(
    mut chat_threads: Signal<Vec<ChatThreadSummary>>,
    mut active_chat_thread_id: Signal<Option<i64>>,
    connection_name: String,
) {
    let _ = acp::disconnect_acp_agent();
    spawn(async move {
        match storage::create_chat_thread(connection_name, Some("New chat".to_string())).await {
            Ok(thread) => {
                chat_threads
                    .with_mut(|threads| upsert_chat_thread_summary(threads, thread.clone()));
                active_chat_thread_id.set(Some(thread.id));
            }
            Err(err) => {
                toast_error(format!("Failed to create chat thread: {err}"));
            }
        }
    });
}

pub fn select_chat_thread(mut active_chat_thread_id: Signal<Option<i64>>, thread_id: i64) {
    if active_chat_thread_id() == Some(thread_id) {
        return;
    }

    let _ = acp::disconnect_acp_agent();
    active_chat_thread_id.set(Some(thread_id));
}

pub fn delete_chat_thread(
    mut chat_threads: Signal<Vec<ChatThreadSummary>>,
    mut active_chat_thread_id: Signal<Option<i64>>,
    connection_name: String,
    thread_id: i64,
) {
    let was_active = active_chat_thread_id() == Some(thread_id);
    let fallback_active = active_chat_thread_id();

    spawn(async move {
        if let Err(err) = storage::delete_chat_thread(thread_id).await {
            toast_error(format!("Failed to delete chat thread: {err}"));
            return;
        }

        let mut next_thread_id = fallback_active.filter(|current| *current != thread_id);
        chat_threads.with_mut(|threads| {
            threads.retain(|thread| thread.id != thread_id);
            if was_active {
                next_thread_id = threads.first().map(|thread| thread.id);
            }
        });

        if was_active {
            let _ = acp::disconnect_acp_agent();
        }

        if let Some(next_thread_id) = next_thread_id {
            active_chat_thread_id.set(Some(next_thread_id));
            return;
        }

        match storage::create_chat_thread(connection_name, Some("New chat".to_string())).await {
            Ok(thread) => {
                chat_threads
                    .with_mut(|threads| upsert_chat_thread_summary(threads, thread.clone()));
                active_chat_thread_id.set(Some(thread.id));
            }
            Err(err) => {
                toast_error(format!("Failed to recreate chat thread: {err}"));
            }
        }
    });
}
