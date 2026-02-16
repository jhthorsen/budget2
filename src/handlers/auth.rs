use askama::Template;
use axum::extract::{Query, State};
use axum::response::IntoResponse;
use crate::{models::*, request_context::RequestContext, AppState};
use crate::models::auth::{OAuthUserInfo, SESSION_USER_KEY};
use oauth2::{AuthorizationCode, TokenResponse, reqwest::async_http_client};
use oauth2::{CsrfToken, Scope};
use serde::Deserialize;

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    ctx: RequestContext,
}

#[derive(Debug, Deserialize)]
pub struct AuthRequest {
    code: String,
    // TODO Need to verify state=xyz
    #[allow(dead_code)]
    state: String,
}

pub async fn callback(
    Query(query): Query<AuthRequest>,
    State(state): State<AppState>,
    session: tower_sessions::Session,
) -> crate::HttpResult {
    let token = state
        .oauth_client
        .exchange_code(AuthorizationCode::new(query.code))
        .request_async(async_http_client)
        .await
        .map_err(|err| super::render_error(&err.to_string(), "Failed to get token"))?;

    let client = reqwest::Client::new();
    let user_info: OAuthUserInfo = client
        .get(&state.userinfo_url)
        .bearer_auth(token.access_token().secret())
        .send()
        .await
        .map_err(|err| super::render_error(&err.to_string(), "Failed to fetch user info"))?
        .json()
        .await
        .map_err(|err| super::render_error(&err.to_string(), "Failed to parse user info"))?;

    let provider = state
        .oauth_client
        .auth_url()
        .url()
        .host_str()
        .unwrap_or("default");

    let user = user_info
        .get_or_create_user(&state.pool, provider)
        .await
        .map_err(|err| super::db_error(err, "Unable to create user"))?;

    session
        .insert(SESSION_USER_KEY, user.id)
        .await
        .map_err(|err| super::render_error(&err.to_string(), "Unable to insert session"))?;

    session
        .save()
        .await
        .map_err(|err| super::render_error(&err.to_string(), "Unable to save session"))?;

    Ok(axum::response::Redirect::to("/dashboard").into_response())
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
