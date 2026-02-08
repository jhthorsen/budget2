use askama::Template;
use axum::{
    extract::State,
    response::{Html, IntoResponse, Redirect, Response},
    Form,
};
use sqlx::SqlitePool;
use tower_sessions::Session;

use crate::{
    auth::{get_current_user, AppState},
    models::*,
};

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
    summary: BudgetSummary,
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
) -> Result<Response, Response> {
    let user = get_current_user(&session, &state.pool).await;

    let user = match user {
        Some(u) => u,
        None => return Ok(Redirect::to("/").into_response()),
    };

    let transactions = sqlx::query_as::<_, TransactionWithCategory>(
        r#"
        SELECT 
            t.id,
            t.amount,
            t.description,
            t.transaction_date,
            t.type as transaction_type,
            c.name as category_name,
            c.color as category_color
        FROM transactions t
        LEFT JOIN categories c ON t.category_id = c.id
        WHERE t.user_id = ?
        ORDER BY t.transaction_date DESC, t.created_at DESC
        LIMIT 50
        "#,
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

    let summary = calculate_summary(&state.pool, user.id).await.map_err(|e| {
        tracing::error!("Database error: {}", e);
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Database error",
        )
            .into_response()
    })?;

    let template = DashboardTemplate {
        user,
        transactions,
        categories,
        summary,
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
        INSERT INTO transactions (user_id, category_id, amount, description, transaction_date, type)
        VALUES (?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(user.id)
    .bind(new_transaction.category_id)
    .bind(new_transaction.amount)
    .bind(&new_transaction.description)
    .bind(&new_transaction.transaction_date)
    .bind(&new_transaction.transaction_type)
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

async fn calculate_summary(pool: &SqlitePool, user_id: i64) -> Result<BudgetSummary, sqlx::Error> {
    #[derive(sqlx::FromRow)]
    struct SummaryRow {
        total_income: f64,
        total_expenses: f64,
    }

    let result = sqlx::query_as::<_, SummaryRow>(
        r#"
        SELECT 
            COALESCE(SUM(CASE WHEN type = 'income' THEN amount ELSE 0 END), 0) as total_income,
            COALESCE(SUM(CASE WHEN type = 'expense' THEN amount ELSE 0 END), 0) as total_expenses
        FROM transactions
        WHERE user_id = ?
        "#,
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    Ok(BudgetSummary {
        total_income: result.total_income,
        total_expenses: result.total_expenses,
        balance: result.total_income - result.total_expenses,
    })
}
