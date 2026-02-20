use crate::models::auth::{OAuthUserInfo, SESSION_USER_KEY};
use crate::{AppState, models::*, request_context::RequestContext};
use askama::Template;
use axum::extract::{Query, State};
use axum::response::IntoResponse;
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
) -> super::HttpResult {
    let token = state
        .oauth_client
        .exchange_code(AuthorizationCode::new(query.code))
        .request_async(async_http_client)
        .await
        .map_err(|err| err.to_string())?;

    let client = reqwest::Client::new();
    let user_info: OAuthUserInfo = client
        .get(&state.userinfo_url)
        .bearer_auth(token.access_token().secret())
        .send()
        .await
        .map_err(|err| err.to_string())?
        .json()
        .await
        .map_err(|err| err.to_string())?;

    let provider = state
        .oauth_client
        .auth_url()
        .url()
        .host_str()
        .unwrap_or("default");

    let user = user_info.get_or_create_user(&state.pool, provider).await?;

    session
        .insert(SESSION_USER_KEY, user.id)
        .await
        .map_err(|err| format!("Unable to insert session: {err}"))?;

    session
        .save()
        .await
        .map_err(|err| format!("Unable to save session: {err}"))?;

    Ok(axum::response::Redirect::to("/dashboard").into_response())
}

pub async fn index_handler(
    State(state): State<AppState>,
    session: tower_sessions::Session,
    ctx: RequestContext,
) -> super::HttpResult {
    if auth::get_current_user(&state.pool, &session).await.is_ok() {
        return Ok(axum::response::Redirect::to("/dashboard").into_response());
    }

    let template = IndexTemplate { ctx };

    Ok(axum::response::Html(template.render()?).into_response())
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
