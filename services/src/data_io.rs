use drivers::clickhouse::execute_text_query;
use models::{DatabaseConnection, QueryPage, TablePreviewSource};
use rust_xlsxwriter::Workbook;
use serde_json::{Map, Value};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};
use tokio::{fs, task::spawn_blocking};

const IMPORT_BATCH_SIZE: usize = 200;

#[derive(Clone, Debug)]
struct CsvImportData {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
}

pub async fn export_query_page_csv(page: QueryPage, path: PathBuf) -> Result<usize, String> {
    spawn_blocking(move || export_query_page_csv_sync(page, path))
        .await
        .map_err(|err| format!("csv export task failed: {err}"))?
}

pub async fn export_query_page_json(page: QueryPage, path: PathBuf) -> Result<usize, String> {
    let row_count = page.rows.len();
    let payload = query_page_to_json(page);
    let json = serde_json::to_string_pretty(&payload)
        .map_err(|err| format!("failed to serialize JSON export: {err}"))?;

    ensure_parent_dir(&path).await?;
    fs::write(&path, json)
        .await
        .map_err(|err| format!("failed to write {}: {err}", path.display()))?;

    Ok(row_count)
}

pub async fn export_query_page_xlsx(page: QueryPage, path: PathBuf) -> Result<usize, String> {
    spawn_blocking(move || export_query_page_xlsx_sync(page, path))
        .await
        .map_err(|err| format!("xlsx export task failed: {err}"))?
}

pub async fn import_csv_into_table(
    connection: DatabaseConnection,
    source: TablePreviewSource,
    path: PathBuf,
) -> Result<u64, String> {
    let import = spawn_blocking(move || read_csv_import_data(path))
        .await
        .map_err(|err| format!("csv import task failed: {err}"))??;

    if import.rows.is_empty() {
        return Ok(0);
    }

    match connection {
        DatabaseConnection::Sqlite(pool) => {
            let mut transaction = pool
                .begin()
                .await
                .map_err(|err| format!("failed to start SQLite import transaction: {err}"))?;

            for chunk in import.rows.chunks(IMPORT_BATCH_SIZE) {
                let sql = build_insert_sql(
                    &source,
                    &import.headers,
                    chunk,
                    quote_sql_identifier,
                    sql_literal,
                );
                sqlx::query(&sql)
                    .execute(&mut *transaction)
                    .await
                    .map_err(|err| format!("SQLite import failed: {err}"))?;
            }

            transaction
                .commit()
                .await
                .map_err(|err| format!("failed to commit SQLite import: {err}"))?;
        }
        DatabaseConnection::Postgres(pool) => {
            let mut transaction = pool
                .begin()
                .await
                .map_err(|err| format!("failed to start PostgreSQL import transaction: {err}"))?;

            for chunk in import.rows.chunks(IMPORT_BATCH_SIZE) {
                let sql = build_insert_sql(
                    &source,
                    &import.headers,
                    chunk,
                    quote_sql_identifier,
                    sql_literal,
                );
                sqlx::query(&sql)
                    .execute(&mut *transaction)
                    .await
                    .map_err(|err| format!("PostgreSQL import failed: {err}"))?;
            }

            transaction
                .commit()
                .await
                .map_err(|err| format!("failed to commit PostgreSQL import: {err}"))?;
        }
        DatabaseConnection::ClickHouse(config) => {
            for chunk in import.rows.chunks(IMPORT_BATCH_SIZE) {
                let sql = build_insert_sql(
                    &source,
                    &import.headers,
                    chunk,
                    quote_clickhouse_identifier,
                    sql_literal,
                );
                execute_text_query(&config, &sql)
                    .await
                    .map_err(|err| format!("ClickHouse import failed: {err}"))?;
            }
        }
    }

    Ok(import.rows.len() as u64)
}

fn export_query_page_csv_sync(page: QueryPage, path: PathBuf) -> Result<usize, String> {
    ensure_parent_dir_sync(&path)?;
    let mut writer = csv::WriterBuilder::new()
        .from_path(&path)
        .map_err(|err| format!("failed to open {} for CSV export: {err}", path.display()))?;

    writer
        .write_record(&page.columns)
        .map_err(|err| format!("failed to write CSV header: {err}"))?;

    for row in &page.rows {
        writer
            .write_record(row)
            .map_err(|err| format!("failed to write CSV row: {err}"))?;
    }

    writer
        .flush()
        .map_err(|err| format!("failed to flush CSV export {}: {err}", path.display()))?;

    Ok(page.rows.len())
}

fn export_query_page_xlsx_sync(page: QueryPage, path: PathBuf) -> Result<usize, String> {
    ensure_parent_dir_sync(&path)?;
    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();

    for (column_index, column_name) in page.columns.iter().enumerate() {
        worksheet
            .write_string(0, column_index as u16, column_name)
            .map_err(|err| format!("failed to write XLSX header: {err}"))?;
    }

    for (row_index, row) in page.rows.iter().enumerate() {
        for (column_index, cell) in row.iter().enumerate() {
            worksheet
                .write_string((row_index + 1) as u32, column_index as u16, cell)
                .map_err(|err| format!("failed to write XLSX cell: {err}"))?;
        }
    }

    workbook
        .save(&path)
        .map_err(|err| format!("failed to save {}: {err}", path.display()))?;

    Ok(page.rows.len())
}

fn query_page_to_json(page: QueryPage) -> Value {
    let rows = page
        .rows
        .into_iter()
        .map(|row| {
            let mut item = Map::with_capacity(page.columns.len());
            for (index, column_name) in page.columns.iter().enumerate() {
                item.insert(
                    column_name.clone(),
                    Value::String(row.get(index).cloned().unwrap_or_default()),
                );
            }
            Value::Object(item)
        })
        .collect::<Vec<_>>();

    Value::Array(rows)
}

fn read_csv_import_data(path: PathBuf) -> Result<CsvImportData, String> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_path(&path)
        .map_err(|err| format!("failed to open {}: {err}", path.display()))?;

    let headers = reader
        .headers()
        .map_err(|err| format!("failed to read CSV header from {}: {err}", path.display()))?
        .iter()
        .enumerate()
        .map(|(index, header)| normalize_header(index, header))
        .collect::<Result<Vec<_>, _>>()?;

    validate_headers(&headers)?;

    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record.map_err(|err| format!("failed to parse CSV row: {err}"))?;
        if record.len() != headers.len() {
            return Err(format!(
                "CSV row has {} columns, expected {}",
                record.len(),
                headers.len()
            ));
        }
        rows.push(record.iter().map(ToString::to_string).collect());
    }

    Ok(CsvImportData { headers, rows })
}

fn validate_headers(headers: &[String]) -> Result<(), String> {
    if headers.is_empty() {
        return Err("CSV import requires a header row".to_string());
    }

    let mut seen = HashSet::new();
    for header in headers {
        if header.is_empty() {
            return Err("CSV header contains an empty column name".to_string());
        }
        if !seen.insert(header.to_ascii_lowercase()) {
            return Err(format!("CSV header contains duplicate column `{header}`"));
        }
    }

    Ok(())
}

fn normalize_header(index: usize, header: &str) -> Result<String, String> {
    let trimmed = if index == 0 {
        header.trim().trim_start_matches('\u{feff}').trim()
    } else {
        header.trim()
    };

    if trimmed.is_empty() {
        return Err("CSV header contains an empty column name".to_string());
    }

    Ok(trimmed.to_string())
}

fn build_insert_sql(
    source: &TablePreviewSource,
    headers: &[String],
    rows: &[Vec<String>],
    quote_identifier_fn: fn(&str) -> String,
    literal_fn: fn(&str) -> String,
) -> String {
    let columns = headers
        .iter()
        .map(|header| quote_identifier_fn(header))
        .collect::<Vec<_>>()
        .join(", ");
    let values = rows
        .iter()
        .map(|row| {
            let cells = row
                .iter()
                .map(|value| literal_fn(value))
                .collect::<Vec<_>>()
                .join(", ");
            format!("({cells})")
        })
        .collect::<Vec<_>>()
        .join(", ");

    format!(
        "insert into {} ({columns}) values {values}",
        source.qualified_name
    )
}

fn quote_sql_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

fn quote_clickhouse_identifier(identifier: &str) -> String {
    format!("`{}`", identifier.replace('`', "``"))
}

fn sql_literal(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.eq_ignore_ascii_case("null") || trimmed == "\\N" {
        "NULL".to_string()
    } else {
        format!("'{}'", value.replace('\'', "''"))
    }
}

async fn ensure_parent_dir(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .await
            .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    }
    Ok(())
}

fn ensure_parent_dir_sync(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    }
    Ok(())
}
