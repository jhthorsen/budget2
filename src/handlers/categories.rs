use crate::{AppState, models::*};
use axum::response::IntoResponse;
use axum::{Form, extract::State};

pub async fn create_category_handler(
    State(state): State<AppState>,
    session: tower_sessions::Session,
    Form(mut category): Form<Category>,
) -> crate::HttpResult {
    let user = auth::get_current_user(&state.pool, &session).await?;
    category.user_id = user.id;
    category
        .create(&state.pool)
        .await
        .map_err(|err| super::db_error(err, "Unable to create category"))?;

    Ok(axum::response::Redirect::to("/settings").into_response())
}
