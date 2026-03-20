mod clickhouse;
mod postgres;
mod sqlite;
mod ssh_tunnel;

pub use clickhouse::ClickHouseForm;
pub use postgres::PostgresForm;
pub use sqlite::SqliteForm;
pub use ssh_tunnel::SshTunnelFields;

pub(super) fn connection_status_class(status: &str) -> &'static str {
    let normalized = status.trim();

    if normalized.starts_with("Error:") {
        "connect-screen__status connect-screen__status--error"
    } else if normalized.eq_ignore_ascii_case("connecting...") {
        "connect-screen__status connect-screen__status--busy"
    } else if normalized.starts_with("Connected") {
        "connect-screen__status connect-screen__status--success"
    } else {
        "connect-screen__status connect-screen__status--hint"
    }
}
