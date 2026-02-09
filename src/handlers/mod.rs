pub mod csv;
pub mod rules;

use askama::Template;
use axum::{
    extract::{Query, State},
    response::{Html, IntoResponse, Redirect, Response},
    Form,
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tower_sessions::Session;

use crate::{
    auth::{get_current_user, AppState},
    models::*,
    filters,
};

#[derive(Debug, Deserialize)]
pub struct PaginationParams {
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_per_page")]
    pub per_page: i64,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct TransactionFilters {
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_per_page")]
    pub per_page: i64,
    #[serde(default, deserialize_with = "empty_string_as_none")]
    pub search: Option<String>,
    #[serde(default, deserialize_with = "empty_string_as_none")]
    pub transaction_type: Option<String>,
    #[serde(default, deserialize_with = "empty_string_as_none")]
    pub category_id: Option<String>,
    #[serde(default, deserialize_with = "empty_string_as_none_i64")]
    pub account_id: Option<i64>,
    #[serde(default, deserialize_with = "empty_string_as_none")]
    pub account: Option<String>,
    #[serde(default, deserialize_with = "empty_string_as_none")]
    pub date_from: Option<String>,
    #[serde(default, deserialize_with = "empty_string_as_none")]
    pub date_to: Option<String>,
}

fn default_page() -> i64 {
    1
}

fn default_per_page() -> i64 {
    20
}

fn empty_string_as_none<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    if s.trim().is_empty() {
        Ok(None)
    } else {
        Ok(Some(s))
    }
}

fn empty_string_as_none_i64<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    if s.trim().is_empty() {
        Ok(None)
    } else {
        s.parse::<i64>().map(Some).map_err(serde::de::Error::custom)
    }
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    user: Option<User>,
}

#[derive(Template)]
#[template(path = "dashboard.html")]
struct DashboardTemplate {
    user: User,
    transactions: Vec<TransactionWithCategory>,
    categories: Vec<Category>,
    accounts: Vec<AccountWithOwnership>,
    summary: BudgetSummary,
    pagination: PaginationInfo,
    filters: TransactionFilters,
}

pub async fn index_handler(
    State(state): State<AppState>,
    session: Session,
) -> Result<Response, Response> {
    let user = get_current_user(&session, &state.pool).await;

    if user.is_some() {
        return Ok(Redirect::to("/dashboard").into_response());
    }

    let template = IndexTemplate { user };
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

pub async fn dashboard_handler(
    State(state): State<AppState>,
    session: Session,
    Query(filters): Query<TransactionFilters>,
) -> Result<Response, Response> {
    let user = get_current_user(&session, &state.pool).await;

    let user = match user {
        Some(u) => u,
        None => return Ok(Redirect::to("/").into_response()),
    };

    let page = filters.page.max(1);
    let per_page = filters.per_page.clamp(10, 100);
    let offset = (page - 1) * per_page;

    // Build dynamic WHERE clause
    let mut where_clauses = vec!["t.user_id = ?".to_string()];
    let mut params: Vec<String> = vec![user.id.to_string()];

    if let Some(ref search) = filters.search {
        if !search.trim().is_empty() {
            where_clauses.push("t.description LIKE ?".to_string());
            params.push(format!("%{}%", search));
        }
    }

    if let Some(ref trans_type) = filters.transaction_type {
        if !trans_type.is_empty() && trans_type != "all" {
            where_clauses.push("t.type = ?".to_string());
            params.push(trans_type.clone());
        }
    }

    if let Some(ref cat_id) = filters.category_id {
        if cat_id == "no_category" {
            where_clauses.push("t.category_id IS NULL".to_string());
        } else {
            where_clauses.push("t.category_id = ?".to_string());
            params.push(cat_id.to_string());
        }
    }

    if let Some(acc_id) = filters.account_id {
        where_clauses.push("t.account_id = ?".to_string());
        params.push(acc_id.to_string());
    }

    // Keep old account filter for backward compatibility
    if let Some(ref account) = filters.account {
        if !account.trim().is_empty() {
            where_clauses.push("t.account = ?".to_string());
            params.push(account.clone());
        }
    }

    if let Some(ref date_from) = filters.date_from {
        if !date_from.trim().is_empty() {
            where_clauses.push("t.transaction_date >= ?".to_string());
            params.push(date_from.clone());
        }
    }

    if let Some(ref date_to) = filters.date_to {
        if !date_to.trim().is_empty() {
            where_clauses.push("t.transaction_date <= ?".to_string());
            params.push(date_to.clone());
        }
    }

    let where_clause = where_clauses.join(" AND ");

    // Get total count for pagination
    let count_query = format!("SELECT COUNT(*) FROM transactions t WHERE {}", where_clause);
    let mut count_query = sqlx::query_scalar::<_, i64>(&count_query);
    for param in &params {
        count_query = count_query.bind(param);
    }
    let total_items = count_query.fetch_one(&state.pool).await.map_err(|e| {
        tracing::error!("Database error: {}", e);
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Database error",
        )
            .into_response()
    })?;

    let total_pages = (total_items + per_page - 1) / per_page;

    // Get transactions
    let select_query = format!(
        r#"
        SELECT 
            t.id,
            t.amount,
            t.description,
            t.transaction_date,
            t.type as transaction_type,
            t.account,
            t.account_id,
            a.name as account_name,
            c.name as category_name,
            c.color as category_color
        FROM transactions t
        LEFT JOIN categories c ON t.category_id = c.id
        LEFT JOIN accounts a ON t.account_id = a.id
        WHERE {}
        ORDER BY t.transaction_date DESC, t.created_at DESC
        LIMIT ? OFFSET ?
        "#,
        where_clause
    );

    let mut query = sqlx::query_as::<_, TransactionWithCategory>(&select_query);
    for param in &params {
        query = query.bind(param);
    }
    let transactions = query
        .bind(per_page)
        .bind(offset)
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

    let categories = sqlx::query_as::<_, Category>("SELECT * FROM categories WHERE user_id = ? ORDER BY name")
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

    // Get accounts accessible to this user with ownership status
    let accounts = sqlx::query_as::<_, AccountWithOwnership>(
        r#"
        SELECT a.id, a.name, a.description, ua.is_mine
        FROM accounts a
        INNER JOIN user_accounts ua ON a.id = ua.account_id
        WHERE ua.user_id = ?
        ORDER BY a.name
        "#
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

    let summary = calculate_summary(&state.pool, user.id).await.map_err(|e| {
        tracing::error!("Database error: {}", e);
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Database error",
        )
            .into_response()
    })?;

    let pagination = PaginationInfo {
        current_page: page,
        total_pages,
        per_page,
        total_items,
    };

    let template = DashboardTemplate {
        user,
        transactions,
        categories,
        accounts,
        summary,
        pagination,
        filters,
    };

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

pub async fn create_transaction_handler(
    State(state): State<AppState>,
    session: Session,
    Form(new_transaction): Form<NewTransaction>,
) -> Result<Redirect, Response> {
    let user = get_current_user(&session, &state.pool)
        .await
        .ok_or_else(|| Redirect::to("/").into_response())?;

    sqlx::query(
        r#"
        INSERT INTO transactions (user_id, category_id, account_id, amount, description, transaction_date, type, account)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(user.id)
    .bind(new_transaction.category_id)
    .bind(new_transaction.account_id)
    .bind(new_transaction.amount)
    .bind(&new_transaction.description)
    .bind(&new_transaction.transaction_date)
    .bind(&new_transaction.transaction_type)
    .bind(&new_transaction.account)
    .execute(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Database error: {}", e);
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Database error",
        )
            .into_response()
    })?;

    Ok(Redirect::to("/dashboard"))
}

pub async fn create_category_handler(
    State(state): State<AppState>,
    session: Session,
    Form(new_category): Form<NewCategory>,
) -> Result<Redirect, Response> {
    let user = get_current_user(&session, &state.pool)
        .await
        .ok_or_else(|| Redirect::to("/").into_response())?;

    sqlx::query(
        "INSERT INTO categories (user_id, name, color) VALUES (?, ?, ?)",
    )
    .bind(user.id)
    .bind(&new_category.name)
    .bind(&new_category.color)
    .execute(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Database error: {}", e);
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Database error",
        )
            .into_response()
    })?;

    Ok(Redirect::to("/dashboard"))
}

pub async fn create_account_handler(
    State(state): State<AppState>,
    session: Session,
    Form(new_account): Form<NewAccount>,
) -> Result<Redirect, Response> {
    let user = get_current_user(&session, &state.pool)
        .await
        .ok_or_else(|| Redirect::to("/").into_response())?;

    // Check if account already exists
    let existing = sqlx::query_scalar::<_, i64>("SELECT id FROM accounts WHERE name = ?")
        .bind(&new_account.name)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!("Database error: {}", e);
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Database error",
            )
                .into_response()
        })?;

    let account_id = if let Some(id) = existing {
        id
    } else {
        // Create new account
        let result = sqlx::query(
            "INSERT INTO accounts (name, description) VALUES (?, ?)",
        )
        .bind(&new_account.name)
        .bind(&new_account.description)
        .execute(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!("Database error: {}", e);
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Database error",
            )
                .into_response()
        })?;
        result.last_insert_rowid()
    };

    // Link user to account
    sqlx::query(
        "INSERT OR IGNORE INTO user_accounts (user_id, account_id) VALUES (?, ?)",
    )
    .bind(user.id)
    .bind(account_id)
    .execute(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Database error: {}", e);
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Database error",
        )
            .into_response()
    })?;

    Ok(Redirect::to("/dashboard"))
}

pub async fn toggle_account_ownership_handler(
    State(state): State<AppState>,
    session: Session,
    Form(toggle): Form<ToggleAccountOwnership>,
) -> Result<Redirect, Response> {
    let user = get_current_user(&session, &state.pool)
        .await
        .ok_or_else(|| Redirect::to("/").into_response())?;

    sqlx::query(
        "UPDATE user_accounts SET is_mine = ? WHERE user_id = ? AND account_id = ?",
    )
    .bind(toggle.is_mine)
    .bind(user.id)
    .bind(toggle.account_id)
    .execute(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Database error: {}", e);
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Database error",
        )
            .into_response()
    })?;

    Ok(Redirect::to("/dashboard"))
}

async fn calculate_summary(pool: &SqlitePool, user_id: i64) -> Result<BudgetSummary, sqlx::Error> {
    #[derive(sqlx::FromRow)]
    struct SummaryRow {
        total_income: f64,
        total_expenses: f64,
    }

    let result = sqlx::query_as::<_, SummaryRow>(
        r#"
        SELECT 
            CAST(COALESCE(SUM(CASE WHEN t.type = 'income' THEN t.amount ELSE 0 END), 0.0) AS REAL) as total_income,
            CAST(COALESCE(SUM(CASE WHEN t.type = 'expense' THEN t.amount ELSE 0 END), 0.0) AS REAL) as total_expenses
        FROM transactions t
        LEFT JOIN user_accounts ua ON t.account_id = ua.account_id AND ua.user_id = ?
        WHERE t.user_id = ? 
          AND (t.account_id IS NULL OR ua.is_mine = 1)
        "#,
    )
    .bind(user_id)
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    Ok(BudgetSummary {
        total_income: result.total_income,
        total_expenses: result.total_expenses,
        balance: result.total_income - result.total_expenses,
    })
}
