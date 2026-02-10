use axum::{
    extract::State,
    response::{IntoResponse, Redirect, Response},
    Form,
};
use tower_sessions::Session;

use crate::{
    auth::{get_current_user, AppState},
    models::{NewAccount, ToggleAccountOwnership},
};

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
