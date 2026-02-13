use crate::{models::*, request_context::RequestContext, AppState};
use askama::Template;
use axum::extract::State;
use axum::response::IntoResponse;
use oauth2::{CsrfToken, Scope};

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    ctx: RequestContext,
}

pub async fn index_handler(
    State(state): State<AppState>,
    session: tower_sessions::Session,
    ctx: RequestContext,
) -> crate::HttpResult {
    if auth::get_current_user(&state.pool, &session).await.is_ok() {
        return Ok(axum::response::Redirect::to("/dashboard").into_response());
    }

    let template = IndexTemplate { ctx };

    Ok(template
        .render()
        .map(axum::response::Html)
        .map_err(super::template_error)?
        .into_response())
}

pub async fn login_handler(State(state): State<crate::AppState>) -> impl IntoResponse {
    let (auth_url, _csrf_token) = state
        .oauth_client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("openid".to_string()))
        .add_scope(Scope::new("email".to_string()))
        .add_scope(Scope::new("profile".to_string()))
        .url();

    axum::response::Redirect::to(auth_url.as_str())
}

pub async fn logout_handler(session: tower_sessions::Session) -> impl IntoResponse {
    session.delete().await.ok();
    axum::response::Redirect::to("/")
}
