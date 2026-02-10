use axum::{
    extract::State,
    response::{IntoResponse, Redirect, Response},
    Form,
};
use tower_sessions::Session;

use crate::{
    auth::{get_current_user, AppState},
    models::NewCategory,
};

pub async fn create_category_handler(
    State(state): State<AppState>,
    session: Session,
    Form(new_category): Form<NewCategory>,
) -> Result<Redirect, Response> {
    let user = get_current_user(&session, &state.pool)
        .await
        .ok_or_else(|| Redirect::to("/").into_response())?;

    sqlx::query(
        "INSERT INTO categories (user_id, name, color) VALUES (?, ?, ?)",
    )
    .bind(user.id)
    .bind(&new_category.name)
    .bind(&new_category.color)
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
