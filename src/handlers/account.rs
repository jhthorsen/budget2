use askama::Template;
use axum::{
    extract::State,
    response::{Html, IntoResponse, Redirect, Response},
};
use tower_sessions::Session;

use crate::{
    auth::{get_current_user, AppState},
    models::*,
    request_context::RequestContext,
};

#[derive(Template)]
#[template(path = "account.html")]
struct AccountTemplate {
    user: User,
    categories: Vec<Category>,
    accounts: Vec<AccountWithOwnership>,
    csr: bool,
    nonce: String,
}

pub async fn account_page(
    State(state): State<AppState>,
    session: Session,
    ctx: RequestContext,
) -> Result<Response, Response> {
    let user = get_current_user(&session, &state.pool)
        .await
        .ok_or_else(|| Redirect::to("/").into_response())?;

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

    let template = AccountTemplate {
        user,
        categories,
        accounts,
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
