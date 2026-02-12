use crate::{models::*, AppState};
use axum::response::IntoResponse;
use axum::{extract::State, Form};

pub async fn create_account_handler(
    State(state): State<AppState>,
    session: tower_sessions::Session,
    Form(new_account): Form<NewAccount>,
) -> crate::HttpResult {
    let user = auth::get_current_user(&session, &state.pool).await?;

    let account_id = sqlx::query_scalar::<_, i64>("SELECT id FROM accounts WHERE name = ?")
        .bind(&new_account.name)
        .fetch_optional(&state.pool)
        .await
        .map_err(|err| super::db_error(err, "Unable to find existing account"))?
        .unwrap_or(
            sqlx::query("INSERT INTO accounts (name, description) VALUES (?, ?)")
                .bind(&new_account.name)
                .bind(&new_account.description)
                .execute(&state.pool)
                .await
                .map_err(|err| super::db_error(err, "Unable to insert new account"))?
                .last_insert_rowid(),
        );

    sqlx::query("INSERT OR IGNORE INTO user_accounts (user_id, account_id) VALUES (?, ?)")
        .bind(user.id)
        .bind(account_id)
        .execute(&state.pool)
        .await
        .map_err(|err| super::db_error(err, "Unable to link account to user"))?;

    Ok(axum::response::Redirect::to("/settings").into_response())
}

pub async fn toggle_account_ownership_handler(
    State(state): State<AppState>,
    session: tower_sessions::Session,
    Form(toggle): Form<ToggleAccountOwnership>,
) -> crate::HttpResult {
    let user = auth::get_current_user(&session, &state.pool).await?;

    sqlx::query("UPDATE user_accounts SET is_mine = ? WHERE user_id = ? AND account_id = ?")
        .bind(toggle.is_mine)
        .bind(user.id)
        .bind(toggle.account_id)
        .execute(&state.pool)
        .await
        .map_err(|err| super::db_error(err, "Unable to toggle account ownership"))?;

    Ok(axum::response::Redirect::to("/settings").into_response())
}
