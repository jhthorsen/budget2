use crate::AppState;
use crate::{models::*, request_context::RequestContext};
use askama::Template;
use axum::response::IntoResponse;
use axum::{Form, extract::State};

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
    let user = auth::get_current_user(&state.pool, &session).await?;

    let template = AddTransactionTemplate {
        accounts: AccountWithOwnership::accounts_for_user(&state.pool, user.id)
            .await
            .map_err(|err| super::db_error(err, "Unable to fetch accounts"))?,
        categories: Category::categories_for_user(&state.pool, user.id)
            .await
            .map_err(|err| super::db_error(err, "Unable to fetch categories"))?,
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
    Form(mut transaction): Form<Transaction>,
) -> crate::HttpResult {
    let user = auth::get_current_user(&state.pool, &session).await?;
    transaction.user_id = user.id;
    transaction
        .create(&state.pool)
        .await
        .map_err(|err| super::db_error(err, "Unable to save new transaction"))?;

    Ok(axum::response::Redirect::to("/dashboard").into_response())
}
