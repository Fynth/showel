use dioxus::prelude::*;
pub mod clickhouse;
pub mod kind_selector;
pub mod postgres;
pub mod sqlite;
use models::*;
#[component]
pub fn DbConnect() -> Element {
    let selected_kind = use_signal(|| DatabaseKind::Sqlite);

    rsx! {
        kind_selector::KindSelector { selected_kind }

        match selected_kind() {
            DatabaseKind::Sqlite => rsx! { sqlite::SqliteForm {} },
            DatabaseKind::Postgres => rsx! { postgres::PostgresForm {} },
            DatabaseKind::ClickHouse => rsx! { clickhouse::ClickHouseForm {} },
        }
    }
}
