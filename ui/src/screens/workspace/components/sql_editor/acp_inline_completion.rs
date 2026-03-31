//! ACP-powered inline completion for SQL editor.
//!
//! This module provides AI-assisted inline completions using the ACP runtime:
//! - Debounced completion requests (100-150ms after keystroke)
//! - Cancellation of pending requests on new keystroke
//! - Ghost text display with Tab to accept, Escape to dismiss
//! - Loading indicator while waiting for ACP response

use std::time::Duration;
use tokio::task::AbortHandle;

/// State for managing ACP inline completion lifecycle.
#[derive(Clone, Default)]
pub struct AcpInlineCompletionState {
    /// Handle to abort any pending ACP request.
    pub pending_request: Option<AbortHandle>,
    /// The current completion suggestion text.
    pub suggestion: Option<String>,
    /// The cursor position where the suggestion should be inserted.
    pub cursor_position: Option<usize>,
    /// Whether the completion has been explicitly discarded.
    pub is_discarded: bool,
    /// Whether we're currently waiting for an ACP response.
    pub is_loading: bool,
    /// Revision number to track stale responses.
    pub request_revision: u64,
}

impl AcpInlineCompletionState {
    /// Creates a new empty state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Shows a completion suggestion.
    pub fn show_completion(&mut self, suggestion: String, cursor_position: usize) {
        self.suggestion = Some(suggestion);
        self.cursor_position = Some(cursor_position);
        self.is_discarded = false;
        self.is_loading = false;
    }

    /// Accepts the current completion.
    /// Returns the accepted text and cursor position if there was a suggestion.
    pub fn accept_completion(&mut self) -> Option<(String, usize)> {
        let suggestion = self.suggestion.take()?;
        let cursor_position = self.cursor_position.take()?;
        self.pending_request = None;
        self.is_discarded = false;
        self.is_loading = false;
        Some((suggestion, cursor_position))
    }

    /// Fully dismisses the completion.
    /// This cancels any pending request and marks as discarded.
    pub fn dismiss_completion(&mut self) {
        if let Some(handle) = self.pending_request.take() {
            handle.abort();
        }
        self.suggestion = None;
        self.cursor_position = None;
        self.is_discarded = true;
        self.is_loading = false;
    }

    /// Cancels any pending request without marking as discarded.
    pub fn cancel_pending(&mut self) {
        if let Some(handle) = self.pending_request.take() {
            handle.abort();
        }
        self.is_loading = false;
    }

    /// Sets the abort handle for a pending request.
    pub fn set_pending_handle(&mut self, handle: AbortHandle, revision: u64) {
        self.pending_request = Some(handle);
        self.request_revision = revision;
        self.is_loading = true;
        self.is_discarded = false;
    }

    /// Returns true if there's an active completion to show.
    pub fn has_completion(&self) -> bool {
        self.suggestion.is_some() && !self.is_discarded && !self.is_loading
    }

    /// Returns true if currently loading a completion.
    pub fn is_loading(&self) -> bool {
        self.is_loading && !self.is_discarded
    }

    /// Returns the current suggestion text if available.
    pub fn suggestion_text(&self) -> Option<&str> {
        if self.is_discarded || self.is_loading {
            return None;
        }
        self.suggestion.as_deref()
    }

    /// Clears the suggestion without discarding (for new keystrokes).
    pub fn clear_suggestion(&mut self) {
        self.suggestion = None;
        self.cursor_position = None;
        self.is_loading = false;
    }
}

/// Configuration for ACP inline completion.
pub struct AcpInlineCompletionConfig {
    /// Debounce delay in milliseconds.
    pub debounce_ms: u64,
    /// Context window size (characters before and after cursor).
    pub context_window: usize,
}

impl Default for AcpInlineCompletionConfig {
    fn default() -> Self {
        Self {
            debounce_ms: 125,    // Between 100-150ms as specified
            context_window: 500, // ±500 chars around cursor
        }
    }
}

/// Extracts context around the cursor position for ACP prompt.
///
/// Returns (text_before_cursor, text_after_cursor, cursor_offset).
pub fn extract_editor_context(
    sql: &str,
    cursor_position: usize,
    window: usize,
) -> (String, String, usize) {
    let cursor_position = cursor_position.min(sql.len());

    // Extract text before cursor (up to window chars)
    let start = cursor_position.saturating_sub(window);
    let text_before = &sql[start..cursor_position];

    // Extract text after cursor (up to window chars)
    let end = (cursor_position + window).min(sql.len());
    let text_after = &sql[cursor_position..end];

    // Calculate the offset of cursor within the context
    let cursor_offset = cursor_position - start;

    (
        text_before.to_string(),
        text_after.to_string(),
        cursor_offset,
    )
}

/// Builds an ACP prompt for inline completion.
///
/// The prompt includes:
/// - Current SQL context around the cursor
/// - Schema hints from the catalog
/// - Instructions for generating a completion
pub fn build_inline_completion_prompt(
    text_before: &str,
    text_after: &str,
    cursor_offset: usize,
    schema_hint: Option<&str>,
) -> String {
    let mut prompt = String::new();

    prompt.push_str(
        "You are a SQL completion assistant. Complete the SQL query at the cursor position.\n\n",
    );
    prompt.push_str("Rules:\n");
    prompt.push_str("1. Return ONLY the text to insert at the cursor position\n");
    prompt.push_str("2. Do NOT include any explanation or markdown\n");
    prompt.push_str("3. Do NOT repeat text that already exists before or after the cursor\n");
    prompt.push_str("4. Keep completions concise and relevant\n");
    prompt.push_str("5. If you cannot determine a good completion, return an empty string\n\n");

    if let Some(hint) = schema_hint {
        prompt.push_str("Schema context:\n");
        prompt.push_str(hint);
        prompt.push_str("\n\n");
    }

    prompt.push_str("SQL context (cursor marked with <|CURSOR|>):\n");
    prompt.push_str("```\n");
    prompt.push_str(text_before);
    prompt.push_str("<|CURSOR|>");
    prompt.push_str(text_after);
    prompt.push_str("\n```\n\n");
    prompt.push_str(&format!(
        "Cursor is at position {} in the context above.\n",
        cursor_offset
    ));
    prompt.push_str("Return the completion text to insert at the cursor position:");

    prompt
}

/// Extracts a schema hint from the catalog for ACP context.
pub fn build_schema_hint(
    schemas: &[String],
    relations: &[super::autocomplete::AutocompleteRelation],
    max_items: usize,
) -> String {
    let mut hint = String::new();

    if !schemas.is_empty() {
        hint.push_str("Available schemas: ");
        hint.push_str(
            &schemas
                .iter()
                .take(5)
                .cloned()
                .collect::<Vec<_>>()
                .join(", "),
        );
        if schemas.len() > 5 {
            hint.push_str(&format!(" (and {} more)", schemas.len() - 5));
        }
        hint.push('\n');
    }

    if !relations.is_empty() {
        hint.push_str("Available tables/views:\n");
        for relation in relations.iter().take(max_items) {
            if let Some(schema) = &relation.schema {
                hint.push_str(&format!(
                    "  - {}.{} ({})\n",
                    schema, relation.name, relation.kind_label
                ));
            } else {
                hint.push_str(&format!(
                    "  - {} ({})\n",
                    relation.name, relation.kind_label
                ));
            }
        }
        if relations.len() > max_items {
            hint.push_str(&format!("  (and {} more)\n", relations.len() - max_items));
        }
    }

    hint
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_acp_inline_completion_state_new() {
        let state = AcpInlineCompletionState::new();
        assert!(state.suggestion.is_none());
        assert!(!state.is_discarded);
        assert!(state.pending_request.is_none());
        assert!(!state.is_loading);
    }

    #[test]
    fn test_show_completion() {
        let mut state = AcpInlineCompletionState::new();
        state.show_completion("SELECT * FROM users".to_string(), 10);
        assert_eq!(state.suggestion, Some("SELECT * FROM users".to_string()));
        assert_eq!(state.cursor_position, Some(10));
        assert!(!state.is_discarded);
        assert!(!state.is_loading);
    }

    #[test]
    fn test_accept_completion() {
        let mut state = AcpInlineCompletionState::new();
        state.show_completion("SELECT * FROM".to_string(), 15);
        let accepted = state.accept_completion();
        assert_eq!(accepted, Some(("SELECT * FROM".to_string(), 15)));
        assert!(state.suggestion.is_none());
        assert!(state.cursor_position.is_none());
        assert!(!state.is_discarded);
    }

    #[test]
    fn test_dismiss_completion() {
        let mut state = AcpInlineCompletionState::new();
        state.show_completion("SELECT".to_string(), 6);
        state.dismiss_completion();
        assert!(state.suggestion.is_none());
        assert!(state.is_discarded);
    }

    #[test]
    fn test_has_completion() {
        let mut state = AcpInlineCompletionState::new();
        assert!(!state.has_completion());

        state.show_completion("test".to_string(), 4);
        assert!(state.has_completion());

        state.dismiss_completion();
        assert!(!state.has_completion());
    }

    #[test]
    fn test_is_loading() {
        let mut state = AcpInlineCompletionState::new();
        assert!(!state.is_loading());

        state.is_loading = true;
        assert!(state.is_loading());

        state.dismiss_completion();
        assert!(!state.is_loading());
    }

    #[test]
    fn test_extract_editor_context() {
        let sql = "SELECT * FROM users WHERE id = 1";
        // Cursor at position 14 (after "SELECT * FROM")
        // Window of 10 chars before and after
        let (before, after, offset) = extract_editor_context(sql, 14, 10);
        // start = max(0, 14 - 10) = 4
        // before = sql[4..14] = "CT * FROM " (10 chars)
        // after = sql[14..24] = "users WHER" (10 chars)
        // offset = 14 - 4 = 10
        assert_eq!(before, "CT * FROM ");
        assert_eq!(after, "users WHER");
        assert_eq!(offset, 10);
    }

    #[test]
    fn test_extract_editor_context_boundary() {
        let sql = "SELECT *";
        let (before, after, offset) = extract_editor_context(sql, 4, 500);
        assert_eq!(before, "SELE");
        assert_eq!(after, "CT *");
        assert_eq!(offset, 4);
    }

    #[test]
    fn test_build_inline_completion_prompt() {
        let prompt = build_inline_completion_prompt(
            "SELECT * FROM ",
            " WHERE id = 1",
            14,
            Some("Schema: public\nTables: users, posts"),
        );

        assert!(prompt.contains("SELECT * FROM"));
        assert!(prompt.contains("<|CURSOR|>"));
        assert!(prompt.contains(" WHERE id = 1"));
        assert!(prompt.contains("Schema: public"));
        assert!(prompt.contains("Tables: users, posts"));
    }

    #[test]
    fn test_config_default() {
        let config = AcpInlineCompletionConfig::default();
        assert!(config.debounce_ms >= 100);
        assert!(config.debounce_ms <= 150);
        assert_eq!(config.context_window, 500);
    }
}
