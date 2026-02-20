use crate::{AppState, models::*};
use askama::Template;
use axum::extract::{Multipart, State};
use axum::{Form, response::IntoResponse};
use csv::{ReaderBuilder, StringRecord};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

const CSV_SESSION_KEY: &str = "csv_upload";

#[derive(Debug, Deserialize)]
pub struct ColumnMapping {
    pub file_id: String,
    pub date_column: String,
    pub income_column: String,
    pub expense_column: String,
    pub description_column: String,
    pub account_column: Option<String>,
    pub account_fixed_name: Option<String>,
    pub category_column: Option<String>,
    pub currency_multiplier: Option<String>,
    #[serde(default)]
    header_map: HashMap<String, usize>,
}

impl ColumnMapping {
    pub fn non_empty_value<'a>(
        &self,
        record: &'a StringRecord,
        col_name: Option<&str>,
    ) -> Result<&'a str, String> {
        let Some(col_name) = col_name else {
            return Err("Can't lookup value without column name".to_string());
        };

        self.header_map
            .get(col_name)
            .and_then(|&idx| record.get(idx))
            .map(|s| {
                let s = s.trim();
                if s.is_empty() {
                    Err(format!("Column {col_name} is empty"))
                } else {
                    Ok(s)
                }
            })
            .unwrap_or(Err(format!("Column {col_name} not found")))
    }

    pub fn value<'a>(
        &self,
        record: &'a StringRecord,
        col_name: Option<&str>,
    ) -> Result<&'a str, String> {
        let Some(col_name) = col_name else {
            return Err("Can't lookup value without column name".to_string());
        };

        self.header_map
            .get(col_name)
            .and_then(|&idx| record.get(idx))
            .ok_or(format!("Column {col_name} not found"))
    }

    pub fn with_header_map(self, header_map: HashMap<String, usize>) -> Self {
        Self { header_map, ..self }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct CsvUploadSession {
    file_id: String,
    file_path: String,
    headers: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ImportError {
    pub row_number: usize,
    pub row_data: String,
    pub error: String,
}

impl From<String> for ImportError {
    fn from(error: String) -> Self {
        ImportError {
            row_number: 0,
            row_data: "".to_string(),
            error,
        }
    }
}

impl From<sqlx::Error> for ImportError {
    fn from(err: sqlx::Error) -> Self {
        ImportError {
            row_number: 0,
            row_data: "".to_string(),
            error: err.to_string(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ImportResult {
    pub total_rows: usize,
    pub successful: usize,
    pub errors: Vec<ImportError>,
}

#[derive(Template)]
#[template(path = "csv_imported.html")]
struct CsvImportedTemplate {
    result: ImportResult,
}

#[derive(Template)]
#[template(path = "csv_mapping.html")]
struct CsvMappingTemplate {
    file_id: String,
    headers: Vec<String>,
}

#[derive(Template)]
#[template(path = "csv_upload.html")]
struct CsvUploadTemplate {}

pub async fn csv_import_handler(
    State(state): State<AppState>,
    session: tower_sessions::Session,
    Form(mapping): Form<ColumnMapping>,
) -> super::HttpResult {
    let user = auth::get_current_user(&state.pool, &session).await?;
    let csv_session: CsvUploadSession = match session.get(CSV_SESSION_KEY).await {
        Ok(Some(s)) => s,
        Ok(None) => return Err("No active session.")?,
        Err(err) => return Err("Request does not match active session.")?,
    };

    if csv_session.file_id != mapping.file_id {
        Err("File ID mismatch")?;
    }

    let file_path = PathBuf::from(&csv_session.file_path);
    let result = import_csv_file(&state, &user, &file_path, mapping).await;

    std::fs::remove_file(&file_path).ok();
    session
        .remove::<CsvUploadSession>(CSV_SESSION_KEY)
        .await
        .ok();

    let result = result?;
    let template = CsvImportedTemplate { result };

    Ok(axum::response::Html(template.render()?).into_response())
}

pub async fn csv_upload_handler(
    State(state): State<AppState>,
    session: tower_sessions::Session,
    mut multipart: Multipart,
) -> super::HttpResult {
    let user = auth::get_current_user(&state.pool, &session).await?;
    let temp_dir = std::env::temp_dir();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|err| format!("Unable to upload: {err}"))?
    {
        if field.name() != Some("csv_file") {
            continue;
        }

        let Some(file_name) = field.file_name() else {
            return Err("CSV file must have a file name")?;
        };

        let file_id = &format!(
            "budget_upload_{}_{}.csv",
            user.id,
            file_name.replace("/", "_").replace(".", "_")
        );

        let file_path = temp_dir.join(file_id);
        let data = field
            .bytes()
            .await
            .map_err(|err| format!("Unable to read file: {err}"))?;

        std::fs::write(&file_path, &data).map_err(|err| format!("Unable to save file: {err}"))?;

        let headers = read_csv_headers(&file_path).map_err(|err| {
            std::fs::remove_file(&file_path).ok();
            format!("Unable to read CSV headers: {err}")
        })?;

        let csv_session = CsvUploadSession {
            file_id: file_id.clone(),
            file_path: file_path.to_string_lossy().to_string(),
            headers: headers.clone(),
        };

        session
            .insert(CSV_SESSION_KEY, csv_session)
            .await
            .map_err(|err| err.to_string())?;

        let template = CsvMappingTemplate {
            file_id: file_id.clone(),
            headers,
        };

        return Ok(axum::response::Html(template.render()?).into_response());
    }

    Err("No file uploaded")?
}

pub async fn csv_upload_page(
    State(state): State<AppState>,
    session: tower_sessions::Session,
) -> super::HttpResult {
    let _user = auth::get_current_user(&state.pool, &session).await?;
    let template = CsvUploadTemplate {};

    Ok(axum::response::Html(template.render()?).into_response())
}

fn read_csv_headers(path: &PathBuf) -> Result<Vec<String>, String> {
    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .from_path(path)
        .map_err(|e| e.to_string())?;

    let headers = reader.headers().map_err(|e| e.to_string())?;
    Ok(headers.iter().map(|h| h.to_string()).collect())
}

async fn import_csv_file(
    state: &AppState,
    user: &User,
    path: &PathBuf,
    mapping: ColumnMapping,
) -> Result<ImportResult, String> {
    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .from_path(path)
        .map_err(|e| e.to_string())?;

    let headers = reader.headers().map_err(|e| e.to_string())?;

    let mapping = mapping.with_header_map(
        headers
            .iter()
            .enumerate()
            .map(|(i, h)| (h.to_string(), i))
            .collect(),
    );

    let mut total_rows = 0;
    let mut successful = 0;
    let mut errors = Vec::new();

    for (row_idx, result) in reader.records().enumerate() {
        let row_number = row_idx + 2;
        total_rows += 1;

        match result {
            Ok(record) => match import_row(state, user, &record, &mapping).await {
                Ok(imported) => {
                    if imported {
                        successful += 1;
                    }
                }
                Err(err) => {
                    errors.push(ImportError {
                        row_number,
                        row_data: format!("{:?}", record),
                        error: err.error,
                    });
                }
            },
            Err(err) => {
                errors.push(ImportError {
                    row_number,
                    row_data: "Failed to read row".to_string(),
                    error: err.to_string(),
                });
            }
        }
    }

    Ok(ImportResult {
        total_rows,
        successful,
        errors,
    })
}

async fn import_row(
    state: &AppState,
    user: &User,
    record: &StringRecord,
    mapping: &ColumnMapping,
) -> Result<bool, ImportError> {
    let mut t = Transaction {
        user_id: user.id,
        description: mapping
            .value(record, Some(&mapping.description_column))?
            .to_string(),
        processed_at: normalize_date(mapping.value(record, Some(&mapping.date_column))?)?,
        ..Transaction::default()
    };

    let account_name = mapping
        .non_empty_value(record, mapping.account_column.as_deref())
        .ok()
        .or(mapping.account_fixed_name.as_deref());

    if let Some(account_name) = account_name {
        t.account_id = Account::by_name(&state.pool, account_name)
            .await?
            .save(&state.pool)
            .await?
            .id;
    }

    let category_name = mapping.non_empty_value(record, mapping.category_column.as_deref());
    if let Ok(category_name) = category_name {
        t.category_id = Some(
            Category::by_name(&state.pool, category_name)
                .await?
                .save(&state.pool)
                .await?
                .id,
        );
    }

    // Apply import rules if category/account not set from CSV
    if t.category_id.is_none() || t.account_id == 0 {
        for rule in ImportRule::all(&state.pool)
            .await
            .map_err(|e| format!("Database error fetching rules: {}", e))?
        {
            rule.apply_to_transaction(&mut t);
        }
    }

    let currency_multiplier = mapping
        .currency_multiplier
        .as_ref()
        .and_then(|m| m.parse::<f64>().ok())
        .unwrap_or(1.0);

    let mut imported = 0;
    if let Ok(amount) = mapping.value(record, Some(&mapping.income_column))
        && let Ok(amount) = normalize_amount(amount)
    {
        t.original_amount = amount;
        t.amount = (amount * currency_multiplier).abs();
        t.transaction_type = if amount > 0.0 {
            "income".to_string()
        } else {
            "expense".to_string()
        };

        // t.create_or_update(&state.pool)
        //     .await
        //     .map_err(|e| format!("Database error inserting transaction: {}", e))?;
        imported += 1;
    }

    if let Ok(amount) = mapping.value(record, Some(&mapping.expense_column))
        && let Ok(amount) = normalize_amount(amount)
    {
        t.original_amount = amount;
        t.amount = (amount * currency_multiplier).abs();
        t.transaction_type = if amount > 0.0 {
            "expense".to_string()
        } else {
            "income".to_string()
        };

        // t.create_or_update(&state.pool)
        //     .await
        //     .map_err(|e| format!("Database error inserting transaction: {}", e))?;
        imported += 1;
    }

    Ok(imported > 0)
}

fn normalize_amount(num: &str) -> Result<f64, String> {
    num.trim()
        .replace(",", "")
        .parse()
        .map_err(|_| format!("Invalid amount: {num}"))
}

fn normalize_date(date_str: &str) -> Result<String, String> {
    let date_str = date_str.trim();

    if date_str.contains('/') {
        let parts: Vec<&str> = date_str.split('/').collect();
        if parts.len() == 3 {
            return Ok(format!("{}-{:0>2}-{:0>2}", parts[0], parts[1], parts[2]));
        }
    }

    if date_str.contains('-') && date_str.len() == 10 {
        return Ok(date_str.to_string());
    }

    Err(format!(
        "Invalid date format: '{}'. Expected YYYY-MM-DD or YYYY/MM/DD",
        date_str
    ))
}
