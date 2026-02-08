use askama::Template;
use axum::{
    extract::{Multipart, State},
    response::{Html, IntoResponse, Redirect, Response},
    Form,
};
use csv::ReaderBuilder;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tower_sessions::Session;
use uuid::Uuid;

use crate::{
    auth::{get_current_user, AppState},
    models::*,
};

const CSV_SESSION_KEY: &str = "csv_upload";

#[derive(Template)]
#[template(path = "csv_upload.html")]
struct CsvUploadTemplate {
    user: User,
}

#[derive(Template)]
#[template(path = "csv_mapping.html")]
struct CsvMappingTemplate {
    user: User,
    file_id: String,
    headers: Vec<String>,
    categories: Vec<Category>,
}

#[derive(Template)]
#[template(path = "csv_result.html")]
struct CsvResultTemplate {
    user: User,
    result: ImportResult,
}

pub async fn csv_upload_page(
    State(state): State<AppState>,
    session: Session,
) -> Result<Response, Response> {
    let user = get_current_user(&session, &state.pool)
        .await
        .ok_or_else(|| Redirect::to("/").into_response())?;

    let template = CsvUploadTemplate { user };
    template
        .render()
        .map(Html)
        .map(|html| html.into_response())
        .map_err(|e| {
            tracing::error!("Template error: {}", e);
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Template error",
            )
                .into_response()
        })
}

pub async fn csv_upload_handler(
    State(state): State<AppState>,
    session: Session,
    mut multipart: Multipart,
) -> Result<Response, Response> {
    let user = get_current_user(&session, &state.pool)
        .await
        .ok_or_else(|| Redirect::to("/").into_response())?;

    let temp_dir = std::env::temp_dir();
    let file_id = Uuid::new_v4().to_string();
    let file_path = temp_dir.join(format!("budget_csv_{}.csv", file_id));

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        tracing::error!("Multipart error: {}", e);
        (
            axum::http::StatusCode::BAD_REQUEST,
            "Failed to read upload",
        )
            .into_response()
    })? {
        if field.name() == Some("csv_file") {
            let data = field.bytes().await.map_err(|e| {
                tracing::error!("Failed to read file data: {}", e);
                (
                    axum::http::StatusCode::BAD_REQUEST,
                    "Failed to read file data",
                )
                    .into_response()
            })?;

            fs::write(&file_path, &data).map_err(|e| {
                tracing::error!("Failed to save file: {}", e);
                (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to save file",
                )
                    .into_response()
            })?;

            let headers = read_csv_headers(&file_path).map_err(|e| {
                let _ = fs::remove_file(&file_path);
                tracing::error!("Failed to read CSV headers: {}", e);
                (
                    axum::http::StatusCode::BAD_REQUEST,
                    format!("Failed to read CSV: {}", e),
                )
                    .into_response()
            })?;

            let csv_session = CsvUploadSession {
                file_id: file_id.clone(),
                file_path: file_path.to_string_lossy().to_string(),
                headers: headers.clone(),
            };

            session
                .insert(CSV_SESSION_KEY, csv_session)
                .await
                .map_err(|e| {
                    tracing::error!("Session error: {}", e);
                    (
                        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                        "Session error",
                    )
                        .into_response()
                })?;

            let categories = sqlx::query_as::<_, Category>(
                "SELECT * FROM categories WHERE user_id = ? ORDER BY name",
            )
            .bind(user.id)
            .fetch_all(&state.pool)
            .await
            .map_err(|e| {
                tracing::error!("Database error: {}", e);
                (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    "Database error",
                )
                    .into_response()
            })?;

            let template = CsvMappingTemplate {
                user,
                file_id,
                headers,
                categories,
            };

            return template
                .render()
                .map(Html)
                .map(|html| html.into_response())
                .map_err(|e| {
                    tracing::error!("Template error: {}", e);
                    (
                        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                        "Template error",
                    )
                        .into_response()
                });
        }
    }

    Err((
        axum::http::StatusCode::BAD_REQUEST,
        "No file uploaded",
    )
        .into_response())
}

pub async fn csv_import_handler(
    State(state): State<AppState>,
    session: Session,
    Form(mapping): Form<ColumnMapping>,
) -> Result<Response, Response> {
    let user = get_current_user(&session, &state.pool)
        .await
        .ok_or_else(|| Redirect::to("/").into_response())?;

    let csv_session: CsvUploadSession = session
        .get(CSV_SESSION_KEY)
        .await
        .map_err(|e| {
            tracing::error!("Session error: {}", e);
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Session error",
            )
                .into_response()
        })?
        .ok_or_else(|| {
            (
                axum::http::StatusCode::BAD_REQUEST,
                "No CSV upload session found",
            )
                .into_response()
        })?;

    if csv_session.file_id != mapping.file_id {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            "File ID mismatch",
        )
            .into_response());
    }

    let file_path = PathBuf::from(&csv_session.file_path);
    let result = import_csv_file(&state, &user, &file_path, &mapping).await;

    fs::remove_file(&file_path).ok();
    session.remove::<CsvUploadSession>(CSV_SESSION_KEY).await.ok();

    let result = result.map_err(|e| {
        tracing::error!("Import error: {}", e);
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Import error: {}", e),
        )
            .into_response()
    })?;

    let template = CsvResultTemplate { user, result };
    template
        .render()
        .map(Html)
        .map(|html| html.into_response())
        .map_err(|e| {
            tracing::error!("Template error: {}", e);
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Template error",
            )
                .into_response()
        })
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
    mapping: &ColumnMapping,
) -> Result<ImportResult, String> {
    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .from_path(path)
        .map_err(|e| e.to_string())?;

    let headers = reader.headers().map_err(|e| e.to_string())?;
    let header_map: HashMap<String, usize> = headers
        .iter()
        .enumerate()
        .map(|(i, h)| (h.to_string(), i))
        .collect();

    let mut total_rows = 0;
    let mut successful = 0;
    let mut errors = Vec::new();
    let mut categories_created = std::collections::HashSet::new();
    let mut accounts_created = std::collections::HashSet::new();

    for (row_idx, result) in reader.records().enumerate() {
        let row_number = row_idx + 2;
        total_rows += 1;

        match result {
            Ok(record) => {
                match import_row(state, user, &record, &header_map, mapping, &mut categories_created, &mut accounts_created).await {
                    Ok(_) => successful += 1,
                    Err(e) => {
                        errors.push(ImportError {
                            row_number,
                            row_data: format!("{:?}", record),
                            error: e,
                        });
                    }
                }
            }
            Err(e) => {
                errors.push(ImportError {
                    row_number,
                    row_data: "Failed to read row".to_string(),
                    error: e.to_string(),
                });
            }
        }
    }

    let mut categories_created_vec: Vec<String> = categories_created.into_iter().collect();
    categories_created_vec.sort();

    let mut accounts_created_vec: Vec<String> = accounts_created.into_iter().collect();
    accounts_created_vec.sort();

    Ok(ImportResult {
        total_rows,
        successful,
        failed: errors.len(),
        errors,
        categories_created: categories_created_vec,
        accounts_created: accounts_created_vec,
    })
}

async fn import_row(
    state: &AppState,
    user: &User,
    record: &csv::StringRecord,
    header_map: &HashMap<String, usize>,
    mapping: &ColumnMapping,
    categories_created: &mut std::collections::HashSet<String>,
    accounts_created: &mut std::collections::HashSet<String>,
) -> Result<(), String> {
    let get_field = |col: &str| -> Result<String, String> {
        header_map
            .get(col)
            .and_then(|&idx| record.get(idx))
            .map(|s| s.to_string())
            .ok_or_else(|| format!("Column '{}' not found", col))
    };

    let date_str = get_field(&mapping.date_column)?;
    let transaction_date = normalize_date(&date_str)?;

    let amount_str = get_field(&mapping.amount_column)?;
    let amount: f64 = amount_str
        .trim()
        .replace(",", "")
        .replace("$", "")
        .parse()
        .map_err(|_| format!("Invalid amount: {}", amount_str))?;

    let description = get_field(&mapping.description_column)?;
    
    // Type: use column value if specified, otherwise use fixed value
    let transaction_type = if let Some(col) = &mapping.type_column {
        if !col.is_empty() {
            get_field(col)?.to_lowercase().trim().to_string()
        } else if let Some(fixed) = &mapping.type_fixed_value {
            fixed.to_lowercase().trim().to_string()
        } else {
            return Err("No type column or fixed value specified".to_string());
        }
    } else if let Some(fixed) = &mapping.type_fixed_value {
        fixed.to_lowercase().trim().to_string()
    } else {
        return Err("No type column or fixed value specified".to_string());
    };

    if transaction_type != "income" && transaction_type != "expense" {
        return Err(format!(
            "Invalid transaction type: {}. Must be 'income' or 'expense'",
            transaction_type
        ));
    }

    // Account: use column value if specified, otherwise use fixed value
    let account_name = if let Some(col) = &mapping.account_column {
        if !col.is_empty() {
            get_field(col).ok()
        } else {
            mapping.account_fixed_value.clone()
        }
    } else {
        mapping.account_fixed_value.clone()
    };

    // Handle account (create if needed and get account_id)
    let account_id = if let Some(acc_name) = &account_name {
        let acc_name = acc_name.trim();
        if !acc_name.is_empty() {
            // Check if account exists
            let existing = sqlx::query_scalar::<_, i64>(
                "SELECT id FROM accounts WHERE name = ?",
            )
            .bind(acc_name)
            .fetch_optional(&state.pool)
            .await
            .map_err(|e| format!("Database error: {}", e))?;

            let acc_id = if let Some(id) = existing {
                id
            } else {
                // Create new account
                let result = sqlx::query(
                    "INSERT INTO accounts (name) VALUES (?)",
                )
                .bind(acc_name)
                .execute(&state.pool)
                .await
                .map_err(|e| format!("Failed to create account '{}': {}", acc_name, e))?;

                accounts_created.insert(acc_name.to_string());
                result.last_insert_rowid()
            };

            // Ensure user has access to this account
            sqlx::query(
                "INSERT OR IGNORE INTO user_accounts (user_id, account_id) VALUES (?, ?)",
            )
            .bind(user.id)
            .bind(acc_id)
            .execute(&state.pool)
            .await
            .map_err(|e| format!("Failed to link account: {}", e))?;

            Some(acc_id)
        } else {
            None
        }
    } else {
        None
    };

    let category_id = if let Some(cat_col) = &mapping.category_column {
        if !cat_col.is_empty() {
            if let Ok(cat_name) = get_field(cat_col) {
                let cat_name = cat_name.trim();
                if !cat_name.is_empty() {
                    // First, try to find existing category
                    let existing = sqlx::query_scalar::<_, i64>(
                        "SELECT id FROM categories WHERE user_id = ? AND name = ?",
                    )
                    .bind(user.id)
                    .bind(cat_name)
                    .fetch_optional(&state.pool)
                    .await
                    .map_err(|e| format!("Database error: {}", e))?;

                    if let Some(cat_id) = existing {
                        Some(cat_id)
                    } else {
                        // Category doesn't exist, create it
                        let result = sqlx::query(
                            "INSERT INTO categories (user_id, name) VALUES (?, ?)",
                        )
                        .bind(user.id)
                        .bind(cat_name)
                        .execute(&state.pool)
                        .await
                        .map_err(|e| format!("Failed to create category '{}': {}", cat_name, e))?;

                        categories_created.insert(cat_name.to_string());
                        Some(result.last_insert_rowid())
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    sqlx::query(
        r#"
        INSERT INTO transactions (user_id, category_id, account_id, amount, description, transaction_date, type, account)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(user.id)
    .bind(category_id)
    .bind(account_id)
    .bind(amount)
    .bind(&description)
    .bind(&transaction_date)
    .bind(&transaction_type)
    .bind(&account_name)
    .execute(&state.pool)
    .await
    .map_err(|e| format!("Database error: {}", e))?;

    Ok(())
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
