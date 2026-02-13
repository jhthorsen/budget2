use crate::{AppState, models::*, request_context::RequestContext};
use askama::Template;
use axum::extract::{Multipart, State};
use axum::{Form, response::IntoResponse};
use csv::{ReaderBuilder, StringRecord};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

const CSV_SESSION_KEY: &str = "csv_upload";

#[derive(Debug, Deserialize)]
pub struct ColumnMapping {
    pub file_id: String,
    pub date_column: String,
    pub amount_column: String,
    pub amount_multiplier: Option<String>,
    pub description_column: String,
    pub transaction_type: String,
    pub account_column: Option<String>,
    pub account_fixed_value: Option<String>,
    pub category_column: Option<String>,
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

#[derive(Template)]
#[template(path = "csv_imported.html")]
struct CsvImportedTemplate {
    ctx: RequestContext,
    result: ImportResult,
    user: User,
}

#[derive(Template)]
#[template(path = "csv_mapping.html")]
struct CsvMappingTemplate {
    categories: Vec<Category>,
    ctx: RequestContext,
    file_id: String,
    headers: Vec<String>,
    user: User,
}

#[derive(Template)]
#[template(path = "csv_upload.html")]
struct CsvUploadTemplate {
    ctx: RequestContext,
    user: User,
}

pub async fn csv_import_handler(
    State(state): State<AppState>,
    session: tower_sessions::Session,
    ctx: RequestContext,
    Form(mapping): Form<ColumnMapping>,
) -> crate::HttpResult {
    let user = auth::get_current_user(&state.pool, &session).await?;
    let csv_session: CsvUploadSession = match session.get(CSV_SESSION_KEY).await {
        Ok(Some(s)) => s,
        Ok(None) => return Err(super::session_error(None)),
        Err(err) => return Err(super::session_error(Some(err))),
    };

    if csv_session.file_id != mapping.file_id {
        return Err(super::render_error("File ID mismatch", ""));
    }

    let file_path = PathBuf::from(&csv_session.file_path);
    let result = import_csv_file(&state, &user, &file_path, mapping).await;

    fs::remove_file(&file_path).ok();
    session
        .remove::<CsvUploadSession>(CSV_SESSION_KEY)
        .await
        .ok();

    let result = result.map_err(|err| super::render_error(&err, &err))?;
    let template = CsvImportedTemplate { ctx, result, user };

    Ok(template
        .render()
        .map(axum::response::Html)
        .map_err(super::template_error)?
        .into_response())
}

pub async fn csv_upload_handler(
    State(state): State<AppState>,
    session: tower_sessions::Session,
    ctx: RequestContext,
    mut multipart: Multipart,
) -> crate::HttpResult {
    let user = auth::get_current_user(&state.pool, &session).await?;
    let temp_dir = std::env::temp_dir();

    while let Some(field) = multipart.next_field().await.map_err(|err| {
        super::render_error(&err.to_string(), "Something is wrong with the upload")
    })? {
        if field.name() != Some("csv_file") {
            continue;
        }

        let Some(file_name) = field.file_name() else {
            return Err(super::render_error("CSV file must have a file name", ""));
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
            .map_err(|err| super::render_error(&err.to_string(), "Failed to read file data"))?;

        fs::write(&file_path, &data)
            .map_err(|err| super::render_error(&err.to_string(), "Failed to save file"))?;

        let headers = read_csv_headers(&file_path).map_err(|err| {
            fs::remove_file(&file_path).ok();
            super::render_error(&err.to_string(), "Failed to read CSV headers")
        })?;

        let csv_session = CsvUploadSession {
            file_id: file_id.clone(),
            file_path: file_path.to_string_lossy().to_string(),
            headers: headers.clone(),
        };

        session
            .insert(CSV_SESSION_KEY, csv_session)
            .await
            .map_err(|err| super::render_error(&err.to_string(), "Failed to insert session"))?;

        let categories = Category::categories_for_user(&state.pool, user.id)
            .await
            .map_err(|err| super::db_error(err, "Unable to fetch categories"))?;

        let template = CsvMappingTemplate {
            ctx,
            user,
            file_id: file_id.clone(),
            headers,
            categories,
        };

        return Ok(template
            .render()
            .map(axum::response::Html)
            .map_err(super::template_error)?
            .into_response());
    }

    Err(super::render_error("No file uploaded", ""))
}

pub async fn csv_upload_page(
    State(state): State<AppState>,
    session: tower_sessions::Session,
    ctx: RequestContext,
) -> crate::HttpResult {
    let user = auth::get_current_user(&state.pool, &session).await?;
    let template = CsvUploadTemplate { ctx, user };

    Ok(template
        .render()
        .map(axum::response::Html)
        .map_err(super::template_error)?
        .into_response())
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
    let mut skipped = 0;
    let mut errors = Vec::new();

    for (row_idx, result) in reader.records().enumerate() {
        let row_number = row_idx + 2;
        total_rows += 1;

        match result {
            Ok(record) => match import_row(state, user, &record, &mapping).await {
                Ok(imported) => {
                    if imported {
                        successful += 1;
                    } else {
                        skipped += 1;
                    }
                }
                Err(e) => {
                    errors.push(ImportError {
                        row_number,
                        row_data: format!("{:?}", record),
                        error: e,
                    });
                }
            },
            Err(e) => {
                errors.push(ImportError {
                    row_number,
                    row_data: "Failed to read row".to_string(),
                    error: e.to_string(),
                });
            }
        }
    }

    Ok(ImportResult {
        total_rows,
        successful,
        failed: errors.len(),
        skipped,
        errors,
    })
}

async fn import_row(
    state: &AppState,
    user: &User,
    record: &StringRecord,
    mapping: &ColumnMapping,
) -> Result<bool, String> {
    let mut t = Transaction {
        id: 0,
        user_id: user.id,
        account_id: None,
        account: None,
        category_id: None,
        amount: 0.0,
        original_amount: normalize_amount(mapping.value(record, Some(&mapping.amount_column))?)?,
        description: mapping
            .value(record, Some(&mapping.description_column))?
            .to_string(),
        transaction_date: normalize_date(mapping.value(record, Some(&mapping.date_column))?)?,
        transaction_type: mapping.transaction_type.to_lowercase().trim().to_string(),
    };

    t.amount = t.original_amount
        * mapping
            .amount_multiplier
            .as_ref()
            .and_then(|m| m.parse::<f64>().ok())
            .unwrap_or(1.0);

    t.transaction_type = match t.transaction_type.as_str() {
        "income" if t.original_amount < 0.0 => "expense".into(),
        "expense" if t.original_amount < 0.0 => "income".into(),
        other => return Err(format!("Unexpected transaction_type {other}")),
    };

    let account_name = mapping
        .non_empty_value(record, mapping.account_column.as_deref())
        .map(|v| Some(v.to_string()))
        .unwrap_or(mapping.account_fixed_value.clone());
    if let Some(account_name) = &account_name {
        t.account_id = Some(
            AccountWithOwnership::ensure(&state.pool, account_name)
                .await
                .map_err(|err| err.to_string())?,
        );
    }

    let category_name = mapping.non_empty_value(record, mapping.category_column.as_deref());
    if let Ok(category_name) = &category_name {
        t.category_id = Some(
            Category::ensure(&state.pool, user.id, category_name)
                .await
                .map_err(|err| err.to_string())?,
        );
    }

    // Apply import rules if category/account not set from CSV
    if t.category_id.is_none() || t.account_id.is_none() {
        for rule in ImportRuleWithNames::rules_for_user(&state.pool, user.id)
            .await
            .map_err(|e| format!("Database error fetching rules: {}", e))?
        {
            rule.apply_to_transaction(&mut t);
        }
    }

    t.create_or_update(&state.pool)
        .await
        .map_err(|e| format!("Database error inserting transaction: {}", e))?;

    Ok(true) // Successfully imported
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
