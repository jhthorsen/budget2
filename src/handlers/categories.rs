use crate::{AppState, models::*};
use axum::response::IntoResponse;
use axum::{Form, extract::State};

pub async fn ensure_category_handler(
    State(state): State<AppState>,
    session: tower_sessions::Session,
    Form(category): Form<Category>,
) -> super::HttpResult {
    let _user = auth::get_current_user(&state.pool, &session).await?;

    Category::load(&state.pool, &category)
        .await?
        .unwrap_or(category)
        .save(&state.pool)
        .await?;

    Ok(axum::response::Redirect::to("/settings").into_response())
}
