use crate::{AppState, models::*};
use axum::response::IntoResponse;
use axum::{Form, extract::State};

pub async fn create_account_handler(
    State(state): State<AppState>,
    session: tower_sessions::Session,
    Form(account): Form<AccountWithOwnership>,
) -> crate::HttpResult {
    let user = auth::get_current_user(&state.pool, &session).await?;
    account
        .create_for_user(&state.pool, user.id)
        .await
        .map_err(|err| super::db_error(err, "Unable to create account"))?;

    Ok(axum::response::Redirect::to("/settings").into_response())
}

pub async fn toggle_account_ownership_handler(
    State(state): State<AppState>,
    session: tower_sessions::Session,
    Form(account): Form<AccountWithOwnership>,
) -> crate::HttpResult {
    let user = auth::get_current_user(&state.pool, &session).await?;
    account
        .attach_to_user(&state.pool, user.id)
        .await
        .map_err(|err| super::db_error(err, "Unable to update account"))?;

    Ok(axum::response::Redirect::to("/settings").into_response())
}
