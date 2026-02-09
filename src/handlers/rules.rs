use askama::Template;
use axum::{
    extract::{Path, State},
    response::{Html, IntoResponse, Redirect, Response},
    Form,
};
use tower_sessions::Session;

use crate::{
    auth::{get_current_user, AppState},
    models::*,
};

#[derive(Template)]
#[template(path = "rules_list.html")]
struct RulesListTemplate {
    user: User,
    grouped_rules: Vec<CategoryGroup>,
}

#[derive(Debug, serde::Serialize)]
struct CategoryGroup {
    category_name: String,
    rules: Vec<ImportRuleWithNames>,
}

#[derive(Template)]
#[template(path = "rules_form.html")]
struct RulesFormTemplate {
    user: User,
    rule: Option<ImportRule>,
    categories: Vec<Category>,
    accounts: Vec<AccountWithOwnership>,
}

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
struct ImportRuleWithNames {
    id: i64,
    #[allow(dead_code)]
    user_id: i64,
    pattern: String,
    #[allow(dead_code)]
    category_id: Option<i64>,
    category_name: Option<String>,
    #[allow(dead_code)]
    account_id: Option<i64>,
    account_name: Option<String>,
    priority: i64,
}

pub async fn rules_list(
    State(state): State<AppState>,
    session: Session,
) -> Result<Response, Response> {
    let user = get_current_user(&session, &state.pool)
        .await
        .ok_or_else(|| Redirect::to("/").into_response())?;

    let rules = sqlx::query_as::<_, ImportRuleWithNames>(
        r#"
        SELECT 
            r.id, r.user_id, r.pattern, r.category_id, c.name as category_name,
            r.account_id, a.name as account_name, r.priority
        FROM import_rules r
        LEFT JOIN categories c ON r.category_id = c.id
        LEFT JOIN accounts a ON r.account_id = a.id
        WHERE r.user_id = ?
        ORDER BY c.name ASC NULLS FIRST, r.priority ASC, r.id ASC
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

    // Group rules by category
    let mut grouped_rules: Vec<CategoryGroup> = Vec::new();
    let mut current_category: Option<String> = None;
    let mut current_rules: Vec<ImportRuleWithNames> = Vec::new();

    for rule in rules {
        let category = rule.category_name.clone().unwrap_or_else(|| "No Category".to_string());
        
        if current_category.is_none() {
            current_category = Some(category.clone());
        }
        
        if Some(category.clone()) != current_category {
            // New category, push the previous group
            if let Some(cat_name) = current_category.take() {
                grouped_rules.push(CategoryGroup {
                    category_name: cat_name,
                    rules: current_rules,
                });
                current_rules = Vec::new();
            }
            current_category = Some(category.clone());
        }
        
        current_rules.push(rule);
    }

    // Push the last group
    if let Some(cat_name) = current_category {
        grouped_rules.push(CategoryGroup {
            category_name: cat_name,
            rules: current_rules,
        });
    }

    let template = RulesListTemplate { 
        user, 
        grouped_rules,
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

pub async fn rules_new_page(
    State(state): State<AppState>,
    session: Session,
) -> Result<Response, Response> {
    let user = get_current_user(&session, &state.pool)
        .await
        .ok_or_else(|| Redirect::to("/").into_response())?;

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

    let accounts = sqlx::query_as::<_, AccountWithOwnership>(
        r#"
        SELECT a.id, a.name, a.description, ua.is_mine
        FROM accounts a
        JOIN user_accounts ua ON a.id = ua.account_id
        WHERE ua.user_id = ?
        ORDER BY a.name
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

    let template = RulesFormTemplate {
        user,
        rule: None,
        categories,
        accounts,
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

pub async fn rules_create(
    State(state): State<AppState>,
    session: Session,
    Form(form): Form<ImportRuleForm>,
) -> Result<Response, Response> {
    let user = get_current_user(&session, &state.pool)
        .await
        .ok_or_else(|| Redirect::to("/").into_response())?;

    let category_id = form
        .category_id
        .and_then(|s| if s.is_empty() { None } else { Some(s) })
        .and_then(|s| s.parse::<i64>().ok());

    let account_id = form
        .account_id
        .and_then(|s| if s.is_empty() { None } else { Some(s) })
        .and_then(|s| s.parse::<i64>().ok());

    let priority = form.priority.parse::<i64>().unwrap_or(999);

    sqlx::query(
        r#"
        INSERT INTO import_rules (user_id, pattern, category_id, account_id, priority)
        VALUES (?, ?, ?, ?, ?)
        "#,
    )
    .bind(user.id)
    .bind(&form.pattern)
    .bind(category_id)
    .bind(account_id)
    .bind(priority)
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

    Ok(Redirect::to("/rules").into_response())
}

pub async fn rules_edit_page(
    State(state): State<AppState>,
    session: Session,
    Path(rule_id): Path<i64>,
) -> Result<Response, Response> {
    let user = get_current_user(&session, &state.pool)
        .await
        .ok_or_else(|| Redirect::to("/").into_response())?;

    let rule = sqlx::query_as::<_, ImportRule>(
        "SELECT * FROM import_rules WHERE id = ? AND user_id = ?",
    )
    .bind(rule_id)
    .bind(user.id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Database error: {}", e);
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Database error",
        )
            .into_response()
    })?
    .ok_or_else(|| {
        (
            axum::http::StatusCode::NOT_FOUND,
            "Rule not found",
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

    let accounts = sqlx::query_as::<_, AccountWithOwnership>(
        r#"
        SELECT a.id, a.name, a.description, ua.is_mine
        FROM accounts a
        JOIN user_accounts ua ON a.id = ua.account_id
        WHERE ua.user_id = ?
        ORDER BY a.name
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

    let template = RulesFormTemplate {
        user,
        rule: Some(rule),
        categories,
        accounts,
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

pub async fn rules_update(
    State(state): State<AppState>,
    session: Session,
    Path(rule_id): Path<i64>,
    Form(form): Form<ImportRuleForm>,
) -> Result<Response, Response> {
    let user = get_current_user(&session, &state.pool)
        .await
        .ok_or_else(|| Redirect::to("/").into_response())?;

    let category_id = form
        .category_id
        .and_then(|s| if s.is_empty() { None } else { Some(s) })
        .and_then(|s| s.parse::<i64>().ok());

    let account_id = form
        .account_id
        .and_then(|s| if s.is_empty() { None } else { Some(s) })
        .and_then(|s| s.parse::<i64>().ok());

    let priority = form.priority.parse::<i64>().unwrap_or(999);

    sqlx::query(
        r#"
        UPDATE import_rules 
        SET pattern = ?, category_id = ?, account_id = ?, priority = ?
        WHERE id = ? AND user_id = ?
        "#,
    )
    .bind(&form.pattern)
    .bind(category_id)
    .bind(account_id)
    .bind(priority)
    .bind(rule_id)
    .bind(user.id)
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

    Ok(Redirect::to("/rules").into_response())
}

pub async fn rules_delete(
    State(state): State<AppState>,
    session: Session,
    Path(rule_id): Path<i64>,
) -> Result<Response, Response> {
    let user = get_current_user(&session, &state.pool)
        .await
        .ok_or_else(|| Redirect::to("/").into_response())?;

    sqlx::query("DELETE FROM import_rules WHERE id = ? AND user_id = ?")
        .bind(rule_id)
        .bind(user.id)
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

    Ok(Redirect::to("/rules").into_response())
}

pub async fn rules_apply(
    State(state): State<AppState>,
    session: Session,
    Path(rule_id): Path<i64>,
) -> Result<Response, Response> {
    let user = get_current_user(&session, &state.pool)
        .await
        .ok_or_else(|| Redirect::to("/").into_response())?;

    // Fetch the specific rule
    let rule = sqlx::query_as::<_, ImportRule>(
        "SELECT * FROM import_rules WHERE id = ? AND user_id = ?",
    )
    .bind(rule_id)
    .bind(user.id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Database error: {}", e);
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Database error",
        )
            .into_response()
    })?
    .ok_or_else(|| {
        (
            axum::http::StatusCode::NOT_FOUND,
            "Rule not found",
        )
            .into_response()
    })?;

    let transactions = sqlx::query_as::<_, Transaction>(
        "SELECT * FROM transactions WHERE user_id = ?",
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

    let mut updated_count = 0;
    let description_lower = rule.pattern.to_lowercase();

    for transaction in transactions {
        if transaction.description.to_lowercase().contains(&description_lower) {
            let mut needs_update = false;
            let mut new_category_id = transaction.category_id;
            let mut new_account_id = transaction.account_id;

            if rule.category_id.is_some() && transaction.category_id.is_none() {
                new_category_id = rule.category_id;
                needs_update = true;
            }

            if rule.account_id.is_some() && transaction.account_id.is_none() {
                new_account_id = rule.account_id;
                needs_update = true;
            }

            if needs_update {
                sqlx::query(
                    "UPDATE transactions SET category_id = ?, account_id = ? WHERE id = ?",
                )
                .bind(new_category_id)
                .bind(new_account_id)
                .bind(transaction.id)
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

                updated_count += 1;
            }
        }
    }

    tracing::info!("Applied rule {} to {} transactions", rule_id, updated_count);
    Ok(Redirect::to("/rules").into_response())
}

