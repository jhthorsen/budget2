use crate::AppState;
use crate::{models::*, request_context::RequestContext};
use askama::Template;
use axum::response::IntoResponse;
use axum::{extract::State, Form};

#[derive(Template)]
#[template(path = "add_transaction.html")]
struct AddTransactionTemplate {
    accounts: Vec<AccountWithOwnership>,
    categories: Vec<Category>,
    ctx: RequestContext,
    user: User,
}

pub async fn add_transaction_page(
    State(state): State<crate::AppState>,
    session: tower_sessions::Session,
    ctx: RequestContext,
) -> crate::HttpResult {
    let user = auth::get_current_user(&session, &state.pool).await?;

    let categories =
        sqlx::query_as::<_, Category>("SELECT * FROM categories WHERE user_id = ? ORDER BY name")
            .bind(user.id)
            .fetch_all(&state.pool)
            .await
            .map_err(|err| super::db_error(err, "Unable to get list of categories"))?;

    let accounts = sqlx::query_as::<_, AccountWithOwnership>(
        r#"
        SELECT a.id, a.name, a.description, ua.is_mine
        FROM accounts a
        INNER JOIN user_accounts ua ON a.id = ua.account_id
        WHERE ua.user_id = ?
        ORDER BY a.name
        "#,
    )
    .bind(user.id)
    .fetch_all(&state.pool)
    .await
    .map_err(|err| super::db_error(err, "Unable to get list of accounts"))?;

    let template = AddTransactionTemplate {
        accounts,
        categories,
        ctx,
        user,
    };

    Ok(template
        .render()
        .map(axum::response::Html)
        .map_err(super::template_error)?
        .into_response())
}

pub async fn create_transaction_handler(
    State(state): State<AppState>,
    session: tower_sessions::Session,
    Form(new_transaction): Form<NewTransaction>,
) -> crate::HttpResult {
    let user = auth::get_current_user(&session, &state.pool).await?;

    sqlx::query(
        r#"
        INSERT INTO transactions (user_id, category_id, account_id, amount, original_amount, description, transaction_date, type, account)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(user.id)
    .bind(new_transaction.category_id)
    .bind(new_transaction.account_id)
    .bind(new_transaction.amount)
    .bind(new_transaction.amount)  // For manual entry, original_amount = amount
    .bind(&new_transaction.description)
    .bind(&new_transaction.transaction_date)
    .bind(&new_transaction.transaction_type)
    .bind(&new_transaction.account)
    .execute(&state.pool)
    .await
    .map_err(|err| super::db_error(err, "Unable to save new transaction"))?;

    Ok(axum::response::Redirect::to("/dashboard").into_response())
}
