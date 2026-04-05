use models::{
    QueryFilter, QueryFilterMode, QueryFilterOperator, QueryFilterRule, QueryOutput, QuerySort,
    QueryTabState,
};

pub fn can_sort_tab(tab: &QueryTabState) -> bool {
    tab.preview_source.is_some() || tab.last_run_sql.as_deref().is_some_and(is_sortable_sql)
}

pub fn can_filter_tab(tab: &QueryTabState) -> bool {
    can_sort_tab(tab)
}

pub fn is_sortable_sql(sql: &str) -> bool {
    matches!(
        sql.split_whitespace().next(),
        Some("select" | "SELECT" | "with" | "WITH")
    )
}

pub fn sort_button_class(active_sort: Option<&QuerySort>, column: &str) -> &'static str {
    match active_sort {
        Some(sort) if sort.column_name == column => {
            "results__sort-button results__sort-button--active"
        }
        _ => "results__sort-button",
    }
}

pub fn sort_indicator(active_sort: Option<&QuerySort>, column: &str) -> &'static str {
    match active_sort {
        Some(sort) if sort.column_name == column && sort.descending => "↓",
        Some(sort) if sort.column_name == column => "↑",
        _ => "↕",
    }
}

pub fn result_columns(result: Option<&QueryOutput>) -> Vec<String> {
    match result {
        Some(QueryOutput::Table(page)) => page.columns.clone(),
        _ => Vec::new(),
    }
}

pub fn filter_draft_from_state(
    active_filter: Option<&QueryFilter>,
    columns: &[String],
) -> QueryFilter {
    let mut filter = active_filter
        .cloned()
        .unwrap_or_else(|| blank_filter(columns));

    if filter.rules.is_empty() {
        filter
            .rules
            .push(blank_rule(default_filter_column(columns)));
    }

    for rule in &mut filter.rules {
        if rule.column_name.trim().is_empty()
            || !columns.iter().any(|column| column == &rule.column_name)
        {
            rule.column_name = default_filter_column(columns);
        }
    }

    filter
}

pub fn filter_sync_key_for_tab(active_tab: Option<&QueryTabState>, columns: &[String]) -> String {
    match active_tab {
        Some(tab) => format!("{}|{:?}|{:?}", tab.id, tab.filter.as_ref(), columns),
        None => "no-tab".to_string(),
    }
}

pub fn row_sync_key_for_tab(
    active_tab: Option<&QueryTabState>,
    result: Option<&QueryOutput>,
) -> String {
    match (active_tab, result) {
        (Some(tab), Some(QueryOutput::Table(page))) => format!(
            "{}|{:?}|{:?}|{}|{}|{}|{}",
            tab.id,
            tab.preview_source
                .as_ref()
                .map(|source| &source.qualified_name),
            tab.last_run_sql.as_ref(),
            page.offset,
            page.rows.len(),
            page.columns.len(),
            tab.pending_table_changes.inserted_rows.len()
        ),
        (Some(tab), _) => format!("{}|no-table", tab.id),
        _ => "no-tab".to_string(),
    }
}

pub fn blank_filter(columns: &[String]) -> QueryFilter {
    QueryFilter {
        mode: QueryFilterMode::And,
        rules: vec![blank_rule(default_filter_column(columns))],
    }
}

pub fn blank_rule(default_column: String) -> QueryFilterRule {
    QueryFilterRule {
        column_name: default_column,
        operator: QueryFilterOperator::Contains,
        value: String::new(),
    }
}

pub fn default_filter_column(columns: &[String]) -> String {
    columns.first().cloned().unwrap_or_default()
}

pub fn has_meaningful_rules(filter: &QueryFilter) -> bool {
    filter.rules.iter().any(|rule| {
        !rule.column_name.trim().is_empty()
            && (!rule.value.trim().is_empty() || rule.operator.is_nullary())
    })
}

pub fn filter_panel_should_auto_open(
    active_filter_present: bool,
    filter_draft: &QueryFilter,
) -> bool {
    active_filter_present || has_meaningful_rules(filter_draft)
}

#[cfg(test)]
pub fn filter_panel_should_collapse_after_clear(
    active_filter_present: bool,
    filter_draft: &QueryFilter,
) -> bool {
    !active_filter_present && !has_meaningful_rules(filter_draft)
}

pub fn update_filter_mode(mut filter_draft: Signal<QueryFilter>, value: String) {
    filter_draft.with_mut(|filter| {
        filter.mode = if value.eq_ignore_ascii_case("or") {
            QueryFilterMode::Or
        } else {
            QueryFilterMode::And
        };
    });
}

pub fn add_filter_rule(mut filter_draft: Signal<QueryFilter>, columns: &[String]) {
    filter_draft.with_mut(|filter| {
        filter
            .rules
            .push(blank_rule(default_filter_column(columns)));
    });
}

pub fn remove_filter_rule(mut filter_draft: Signal<QueryFilter>, index: usize, columns: &[String]) {
    filter_draft.with_mut(|filter| {
        if index < filter.rules.len() {
            filter.rules.remove(index);
        }
        if filter.rules.is_empty() {
            filter
                .rules
                .push(blank_rule(default_filter_column(columns)));
        }
    });
}

pub fn update_filter_rule_column(
    mut filter_draft: Signal<QueryFilter>,
    index: usize,
    column_name: String,
) {
    filter_draft.with_mut(|filter| {
        if let Some(rule) = filter.rules.get_mut(index) {
            rule.column_name = column_name;
        }
    });
}

pub fn update_filter_rule_operator(
    mut filter_draft: Signal<QueryFilter>,
    index: usize,
    operator_value: String,
) {
    filter_draft.with_mut(|filter| {
        if let Some(rule) = filter.rules.get_mut(index) {
            rule.operator = parse_filter_operator(&operator_value);
            if rule.operator.is_nullary() {
                rule.value.clear();
            }
        }
    });
}

pub fn update_filter_rule_value(
    mut filter_draft: Signal<QueryFilter>,
    index: usize,
    value: String,
) {
    filter_draft.with_mut(|filter| {
        if let Some(rule) = filter.rules.get_mut(index) {
            rule.value = value;
        }
    });
}

pub fn supported_filter_operators() -> [QueryFilterOperator; 8] {
    [
        QueryFilterOperator::Contains,
        QueryFilterOperator::NotContains,
        QueryFilterOperator::Equals,
        QueryFilterOperator::NotEquals,
        QueryFilterOperator::StartsWith,
        QueryFilterOperator::EndsWith,
        QueryFilterOperator::IsNull,
        QueryFilterOperator::IsNotNull,
    ]
}

pub fn filter_mode_value(mode: QueryFilterMode) -> &'static str {
    match mode {
        QueryFilterMode::And => "and",
        QueryFilterMode::Or => "or",
    }
}

pub fn filter_operator_value(operator: QueryFilterOperator) -> &'static str {
    match operator {
        QueryFilterOperator::Contains => "contains",
        QueryFilterOperator::NotContains => "not_contains",
        QueryFilterOperator::Equals => "equals",
        QueryFilterOperator::NotEquals => "not_equals",
        QueryFilterOperator::StartsWith => "starts_with",
        QueryFilterOperator::EndsWith => "ends_with",
        QueryFilterOperator::IsNull => "is_null",
        QueryFilterOperator::IsNotNull => "is_not_null",
    }
}

pub fn filter_operator_label(operator: QueryFilterOperator) -> &'static str {
    match operator {
        QueryFilterOperator::Contains => "Contains",
        QueryFilterOperator::NotContains => "Does not contain",
        QueryFilterOperator::Equals => "Equals",
        QueryFilterOperator::NotEquals => "Does not equal",
        QueryFilterOperator::StartsWith => "Starts with",
        QueryFilterOperator::EndsWith => "Ends with",
        QueryFilterOperator::IsNull => "Is null",
        QueryFilterOperator::IsNotNull => "Is not null",
    }
}

pub fn parse_filter_operator(value: &str) -> QueryFilterOperator {
    match value {
        "not_contains" => QueryFilterOperator::NotContains,
        "equals" => QueryFilterOperator::Equals,
        "not_equals" => QueryFilterOperator::NotEquals,
        "starts_with" => QueryFilterOperator::StartsWith,
        "ends_with" => QueryFilterOperator::EndsWith,
        "is_null" => QueryFilterOperator::IsNull,
        "is_not_null" => QueryFilterOperator::IsNotNull,
        _ => QueryFilterOperator::Contains,
    }
}

pub fn format_row_json(columns: &[String], row: &[String]) -> String {
    let mut object = serde_json::Map::with_capacity(columns.len());
    for (column, value) in columns.iter().zip(row.iter()) {
        object.insert(column.clone(), detail_json_value(value));
    }

    serde_json::to_string_pretty(&serde_json::Value::Object(object))
        .unwrap_or_else(|_| "{}".to_string())
}

pub fn detail_json_value(value: &str) -> serde_json::Value {
    let trimmed = value.trim();
    if trimmed.eq_ignore_ascii_case("null") {
        serde_json::Value::Null
    } else if (trimmed.starts_with('{') && trimmed.ends_with('}'))
        || (trimmed.starts_with('[') && trimmed.ends_with(']'))
    {
        serde_json::from_str::<serde_json::Value>(trimmed)
            .unwrap_or_else(|_| serde_json::Value::String(value.to_string()))
    } else {
        serde_json::Value::String(value.to_string())
    }
}

pub fn pending_changes_summary(pending_changes: &models::PendingTableChanges) -> String {
    let inserts = pending_changes.inserted_rows.len();
    let updates = pending_changes.updated_cells.len();
    let deletes = pending_changes.deleted_rows.len();
    let mut parts = Vec::new();
    if inserts > 0 {
        parts.push(if inserts == 1 {
            "1 insert".to_string()
        } else {
            format!("{inserts} inserts")
        });
    }
    if updates > 0 {
        parts.push(if updates == 1 {
            "1 update".to_string()
        } else {
            format!("{updates} updates")
        });
    }
    if deletes > 0 {
        parts.push(if deletes == 1 {
            "1 delete".to_string()
        } else {
            format!("{deletes} deletes")
        });
    }
    if parts.is_empty() {
        "No pending changes".to_string()
    } else {
        format!("{} pending", parts.join(", "))
    }
}
