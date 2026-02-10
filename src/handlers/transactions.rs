use axum::{
    extract::State,
    response::{IntoResponse, Redirect, Response},
    Form,
};
use tower_sessions::Session;

use crate::{
    auth::{get_current_user, AppState},
    models::NewTransaction,
};

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
