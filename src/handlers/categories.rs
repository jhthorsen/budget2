use crate::{models::*, AppState};
use axum::response::IntoResponse;
use axum::{extract::State, Form};

pub async fn create_category_handler(
    State(state): State<AppState>,
    session: tower_sessions::Session,
    Form(new_category): Form<NewCategory>,
) -> crate::HttpResult {
    let user = auth::get_current_user(&session, &state.pool).await?;

    sqlx::query("INSERT INTO categories (user_id, name, color) VALUES (?, ?, ?)")
        .bind(user.id)
        .bind(&new_category.name)
        .bind(&new_category.color)
        .execute(&state.pool)
        .await
        .map_err(|err| super::db_error(err, "Unable to create category"))?;

    Ok(axum::response::Redirect::to("/settings").into_response())
}
