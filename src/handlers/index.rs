use askama::Template;
use axum::{
    extract::State,
    response::{Html, IntoResponse, Redirect, Response},
};
use tower_sessions::Session;

use crate::{
    auth::{get_current_user, AppState},
    models::User,
};

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    user: Option<User>,
}

pub async fn index_handler(
    State(state): State<AppState>,
    session: Session,
) -> Result<Response, Response> {
    let user = get_current_user(&session, &state.pool).await;

    if user.is_some() {
        return Ok(Redirect::to("/dashboard").into_response());
    }

    let template = IndexTemplate { user };
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
