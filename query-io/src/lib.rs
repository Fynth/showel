use driver_clickhouse::execute_text_query;
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

pub async fn export_query_page_xml(page: QueryPage, path: PathBuf) -> Result<usize, String> {
    spawn_blocking(move || export_query_page_xml_sync(page, path))
        .await
        .map_err(|err| format!("xml export task failed: {err}"))?
}

pub async fn export_query_page_html(page: QueryPage, path: PathBuf) -> Result<usize, String> {
    spawn_blocking(move || export_query_page_html_sync(page, path))
        .await
        .map_err(|err| format!("html export task failed: {err}"))?
}

pub async fn export_query_page_sql_dump(
    page: QueryPage,
    path: PathBuf,
    table_name: String,
) -> Result<usize, String> {
    spawn_blocking(move || export_query_page_sql_dump_sync(page, path, table_name))
        .await
        .map_err(|err| format!("sql dump export task failed: {err}"))?
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
        DatabaseConnection::MySql(pool) => {
            let mut transaction = pool
                .begin()
                .await
                .map_err(|err| format!("failed to start MySQL import transaction: {err}"))?;

            for chunk in import.rows.chunks(IMPORT_BATCH_SIZE) {
                let sql = build_insert_sql(
                    &source,
                    &import.headers,
                    chunk,
                    quote_clickhouse_identifier,
                    sql_literal,
                );
                sqlx::query(&sql)
                    .execute(&mut *transaction)
                    .await
                    .map_err(|err| format!("MySQL import failed: {err}"))?;
            }

            transaction
                .commit()
                .await
                .map_err(|err| format!("failed to commit MySQL import: {err}"))?;
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

fn export_query_page_xml_sync(page: QueryPage, path: PathBuf) -> Result<usize, String> {
    ensure_parent_dir_sync(&path)?;
    let mut output = String::new();
    output.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    output.push_str("<table>\n");

    for row in &page.rows {
        output.push_str("  <row>\n");
        for (i, cell) in row.iter().enumerate() {
            let col_name = page
                .columns
                .get(i)
                .cloned()
                .unwrap_or_else(|| "column".to_string());
            let escaped = escape_xml(cell);
            output.push_str(&format!("    <{}>{}</{}>\n", col_name, escaped, col_name));
        }
        output.push_str("  </row>\n");
    }

    output.push_str("</table>\n");

    std::fs::write(&path, output)
        .map_err(|err| format!("failed to write {}: {err}", path.display()))?;

    Ok(page.rows.len())
}

fn export_query_page_html_sync(page: QueryPage, path: PathBuf) -> Result<usize, String> {
    ensure_parent_dir_sync(&path)?;
    let mut output = String::new();
    output.push_str("<!DOCTYPE html>\n");
    output.push_str("<html lang=\"en\">\n<head>\n");
    output.push_str("  <meta charset=\"UTF-8\">\n");
    output
        .push_str("  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n");
    output.push_str("  <title>Query Results</title>\n");
    output.push_str("  <style>\n");
    output.push_str("    body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; ");
    output.push_str("margin: 20px; background: #f5f5f5; }\n");
    output.push_str("    table { border-collapse: collapse; width: 100%; background: white; ");
    output.push_str("box-shadow: 0 1px 3px rgba(0,0,0,0.1); }\n");
    output.push_str("    th, td { border: 1px solid #ddd; padding: 10px; text-align: left; ");
    output.push_str("font-size: 14px; }\n");
    output
        .push_str("    th { background: #f8f9fa; font-weight: 600; position: sticky; top: 0; }\n");
    output.push_str("    tr:hover { background: #f8f9fa; }\n");
    output.push_str("    tr:nth-child(even) { background: #fafafa; }\n");
    output.push_str("  </style>\n");
    output.push_str("</head>\n<body>\n");
    output.push_str("  <table>\n");
    output.push_str("    <thead>\n      <tr>\n");
    for col in &page.columns {
        output.push_str(&format!("        <th>{}</th>\n", escape_html(col)));
    }
    output.push_str("      </tr>\n    </thead>\n    <tbody>\n");
    for row in &page.rows {
        output.push_str("      <tr>\n");
        for cell in row {
            output.push_str(&format!("        <td>{}</td>\n", escape_html(cell)));
        }
        output.push_str("      </tr>\n");
    }
    output.push_str("    </tbody>\n  </table>\n</body>\n</html>\n");

    std::fs::write(&path, output)
        .map_err(|err| format!("failed to write {}: {err}", path.display()))?;

    Ok(page.rows.len())
}

fn export_query_page_sql_dump_sync(
    page: QueryPage,
    path: PathBuf,
    table_name: String,
) -> Result<usize, String> {
    ensure_parent_dir_sync(&path)?;
    let mut output = String::new();

    let columns = page
        .columns
        .iter()
        .map(|c| quote_sql_identifier(c))
        .collect::<Vec<_>>()
        .join(", ");

    for row in &page.rows {
        let values = row
            .iter()
            .map(|v| sql_literal(v))
            .collect::<Vec<_>>()
            .join(", ");
        output.push_str(&format!(
            "INSERT INTO {} ({}) VALUES ({});\n",
            quote_sql_identifier(&table_name),
            columns,
            values
        ));
    }

    std::fs::write(&path, output)
        .map_err(|err| format!("failed to write {}: {err}", path.display()))?;

    Ok(page.rows.len())
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
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
