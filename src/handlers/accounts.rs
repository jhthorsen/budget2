use crate::{AppState, models::*};
use axum::response::IntoResponse;
use axum::{Form, extract::State};

pub async fn ensure_account_handler(
    State(state): State<AppState>,
    session: tower_sessions::Session,
    Form(account): Form<Account>,
) -> super::HttpResult {
    let _user = auth::get_current_user(&state.pool, &session).await?;

    Account::load(&state.pool, &account)
        .await?
        .unwrap_or(account)
        .save(&state.pool)
        .await?;

    Ok(axum::response::Redirect::to("/settings").into_response())
}
