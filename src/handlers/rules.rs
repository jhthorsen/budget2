use crate::{AppState, models::*, request_context::RequestContext};
use askama::Template;
use axum::Form;
use axum::extract::{Path, State};
use axum::response::IntoResponse;

#[derive(Template)]
#[template(path = "rules_list.html")]
struct RulesListTemplate {
    ctx: RequestContext,
    grouped_rules: Vec<CategoryGroup>,
    user: User,
}

#[derive(Debug, serde::Serialize)]
struct CategoryGroup {
    category_name: String,
    rules: Vec<ImportRuleWithNames>,
}

#[derive(Template)]
#[template(path = "rules_form.html")]
struct RulesFormTemplate {
    accounts: Vec<AccountWithOwnership>,
    categories: Vec<Category>,
    ctx: RequestContext,
    rule: Option<ImportRuleWithNames>,
    user: User,
}

pub async fn rules_list(
    State(state): State<AppState>,
    session: tower_sessions::Session,
    ctx: RequestContext,
) -> super::HttpResult {
    let user = auth::get_current_user(&state.pool, &session).await?;
    let rules = ImportRuleWithNames::rules_for_user(&state.pool, user.id).await?;

    // Group rules by category
    let mut grouped_rules: Vec<CategoryGroup> = Vec::new();
    let mut current_category: Option<String> = None;
    let mut current_rules: Vec<ImportRuleWithNames> = Vec::new();

    for rule in rules {
        let category = rule
            .category_name
            .clone()
            .unwrap_or_else(|| "No Category".to_string());

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
        ctx,
        user,
        grouped_rules,
    };

    Ok(axum::response::Html(template.render()?).into_response())
}

pub async fn rules_new_page(
    State(state): State<AppState>,
    session: tower_sessions::Session,
    ctx: RequestContext,
) -> super::HttpResult {
    let user = auth::get_current_user(&state.pool, &session).await?;
    let categories = Category::categories_for_user(&state.pool, user.id).await?;
    let accounts = AccountWithOwnership::accounts_for_user(&state.pool, user.id).await?;
    let template = RulesFormTemplate {
        accounts,
        categories,
        ctx,
        rule: None,
        user,
    };

    Ok(axum::response::Html(template.render()?).into_response())
}

pub async fn rules_create(
    State(state): State<AppState>,
    session: tower_sessions::Session,
    Form(form): Form<ImportRuleForm>,
) -> super::HttpResult {
    let user = auth::get_current_user(&state.pool, &session).await?;
    let mut rule = ImportRuleWithNames {
        category_id: form
            .category_id
            .and_then(|s| if s.is_empty() { None } else { Some(s) })
            .and_then(|s| s.parse::<i64>().ok()),
        account_id: form
            .account_id
            .and_then(|s| if s.is_empty() { None } else { Some(s) })
            .and_then(|s| s.parse::<i64>().ok()),
        user_id: user.id,
        pattern: form.pattern,
        priority: form.priority.parse::<i64>().unwrap_or(999),
        ..ImportRuleWithNames::default()
    };

    rule.create(&state.pool).await?;

    Ok(axum::response::Redirect::to("/rules").into_response())
}

pub async fn rules_edit_page(
    State(state): State<AppState>,
    session: tower_sessions::Session,
    Path(rule_id): Path<i64>,
    ctx: RequestContext,
) -> super::HttpResult {
    let user = auth::get_current_user(&state.pool, &session).await?;
    let rule = ImportRuleWithNames::get(&state.pool, user.id, rule_id)
        .await?
        .ok_or_else(|| (axum::http::StatusCode::NOT_FOUND, "Rule not found").into_response())?;
    let categories = Category::categories_for_user(&state.pool, user.id).await?;
    let accounts = AccountWithOwnership::accounts_for_user(&state.pool, user.id).await?;

    let template = RulesFormTemplate {
        accounts,
        categories,
        ctx,
        rule: Some(rule),
        user,
    };

    Ok(axum::response::Html(template.render()?).into_response())
}

pub async fn rules_update(
    State(state): State<AppState>,
    session: tower_sessions::Session,
    Path(rule_id): Path<i64>,
    Form(form): Form<ImportRuleForm>,
) -> super::HttpResult {
    let user = auth::get_current_user(&state.pool, &session).await?;

    let mut rule = ImportRuleWithNames {
        id: rule_id,
        user_id: user.id,
        category_id: form
            .category_id
            .and_then(|s| if s.is_empty() { None } else { Some(s) })
            .and_then(|s| s.parse::<i64>().ok()),
        account_id: form
            .account_id
            .and_then(|s| if s.is_empty() { None } else { Some(s) })
            .and_then(|s| s.parse::<i64>().ok()),
        pattern: form.pattern,
        priority: form.priority.parse::<i64>().unwrap_or(999),
        ..ImportRuleWithNames::default()
    };

    rule.update(&state.pool)
        .await
        .map_err(|err| super::db_error(err, "Unable to save rule"))?;

    Ok(axum::response::Redirect::to("/rules").into_response())
}

pub async fn rules_delete(
    State(state): State<AppState>,
    session: tower_sessions::Session,
    Path(rule_id): Path<i64>,
) -> super::HttpResult {
    let user = auth::get_current_user(&state.pool, &session).await?;
    let mut rule = ImportRuleWithNames {
        id: rule_id,
        user_id: user.id,
        ..ImportRuleWithNames::default()
    };

    rule.delete(&state.pool)
        .await
        .map_err(|err| super::db_error(err, "Unable to delete rule"))?;

    Ok(axum::response::Redirect::to("/rules").into_response())
}

pub async fn apply_rule_to_transactions(
    State(state): State<AppState>,
    session: tower_sessions::Session,
    Path(rule_id): Path<i64>,
) -> super::HttpResult {
    let user = auth::get_current_user(&state.pool, &session).await?;
    let rule = ImportRuleWithNames::get(&state.pool, user.id, rule_id)
        .await
        .map_err(|err| super::db_error(err, "Unable to find rule"))?
        .ok_or_else(|| super::render_error("Rule not found", ""))?;
    let mut transactions = Transaction::transactions_for_user(&state.pool, user.id)
        .await
        .map_err(|err| super::db_error(err, "Unable to find transactions"))?;

    let mut updated_count = 0;
    for t in transactions.iter_mut() {
        if rule.apply_to_transaction(t) {
            t.create_or_update(&state.pool)
                .await
                .map_err(|err| super::db_error(err, "Unable to update transaction"))?;
            updated_count += 1;
        }
    }

    tracing::info!(rule_id, updated_count);
    Ok(axum::response::Redirect::to("/rules").into_response())
}
