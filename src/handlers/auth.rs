use crate::AppState;
use axum::extract::{Query, State};
use axum::response::IntoResponse;
use oauth2::TokenResponse;
use oauth2::{CsrfToken, Scope};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct AuthRequest {
    code: String,
    #[allow(dead_code)]
    state: String,
}

pub async fn callback(
    Query(query): Query<AuthRequest>,
    State(state): State<AppState>,
    session: tower_sessions::Session,
) -> Result<impl IntoResponse, String> {
    let token = state
        .oauth_client
        .exchange_code(oauth2::AuthorizationCode::new(query.code))
        .request_async(oauth2::reqwest::async_http_client)
        .await
        .map_err(|e| format!("Failed to get token: {}", e))?;

    let client = reqwest::Client::new();
    let user_info: crate::auth::OAuthUserInfo = client
        .get(&state.userinfo_url)
        .bearer_auth(token.access_token().secret())
        .send()
        .await
        .map_err(|e| format!("Failed to fetch user info: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse user info: {}", e))?;

    let user = crate::auth::get_or_create_user(&state.pool, &user_info)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

    session
        .insert(crate::auth::SESSION_USER_KEY, user.id)
        .await
        .map_err(|e| format!("Session error: {}", e))?;

    session
        .save()
        .await
        .map_err(|e| format!("Session save error: {}", e))?;

    Ok(axum::response::Redirect::to("/dashboard"))
}

pub async fn login(State(state): State<AppState>) -> impl IntoResponse {
    let (auth_url, _csrf_token) = state
        .oauth_client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("openid".to_string()))
        .add_scope(Scope::new("email".to_string()))
        .add_scope(Scope::new("profile".to_string()))
        .url();

    axum::response::Redirect::to(auth_url.as_str())
}

pub async fn logout(session: tower_sessions::Session) -> impl IntoResponse {
    session.delete().await.ok();
    axum::response::Redirect::to("/")
}
