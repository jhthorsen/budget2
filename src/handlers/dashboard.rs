use askama::Template;
use axum::{
    extract::{Query, State},
    response::{Html, IntoResponse, Redirect, Response},
};
use sqlx::SqlitePool;
use tower_sessions::Session;

use crate::{
    auth::{get_current_user, AppState},
    models::*,
    filters,
    request_context::RequestContext,
};

use super::TransactionFilters;

#[derive(Template)]
#[template(path = "dashboard.html")]
struct DashboardTemplate {
    user: User,
    transactions: Vec<TransactionWithCategory>,
    categories: Vec<Category>,
    accounts: Vec<AccountWithOwnership>,
    summary: BudgetSummary,
    filtered_summary: BudgetSummary,
    pagination: PaginationInfo,
    filters: TransactionFilters,
    csr: bool,
    nonce: String,
}

pub async fn dashboard_handler(
    State(state): State<AppState>,
    session: Session,
    Query(filters): Query<TransactionFilters>,
    ctx: RequestContext,
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

    // Calculate filtered summary
    let filtered_summary_query = format!(
        r#"
        SELECT 
            CAST(COALESCE(SUM(CASE WHEN t.type = 'income' THEN t.amount ELSE 0 END), 0.0) AS REAL) as total_income,
            CAST(COALESCE(SUM(CASE WHEN t.type = 'expense' THEN t.amount ELSE 0 END), 0.0) AS REAL) as total_expenses
        FROM transactions t
        WHERE {}
        "#,
        where_clause
    );

    #[derive(sqlx::FromRow)]
    struct FilteredSummaryRow {
        total_income: f64,
        total_expenses: f64,
    }

    let mut filtered_summary_query_exec = sqlx::query_as::<_, FilteredSummaryRow>(&filtered_summary_query);
    for param in &params {
        filtered_summary_query_exec = filtered_summary_query_exec.bind(param);
    }
    let filtered_result = filtered_summary_query_exec.fetch_one(&state.pool).await.map_err(|e| {
        tracing::error!("Database error: {}", e);
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Database error",
        )
            .into_response()
    })?;

    let filtered_summary = BudgetSummary {
        total_income: filtered_result.total_income,
        total_expenses: filtered_result.total_expenses,
        balance: filtered_result.total_income - filtered_result.total_expenses,
    };

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
        filtered_summary,
        pagination,
        filters,
        csr: ctx.csr,
        nonce: ctx.nonce,
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
