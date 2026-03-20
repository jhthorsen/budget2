use crate::helpers::*;
use oauth2::{AuthorizationCode, CsrfToken, Scope, TokenResponse, reqwest::async_http_client};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct AuthRequest {
    code: String,
    // TODO Need to verify state=xyz
    #[allow(dead_code)]
    state: String,
}

#[derive(Debug, Deserialize)]
struct UserInfo {
    pub email: String,
    pub name: Option<String>,
    #[serde(alias = "sub")]
    pub id: String,
}

pub async fn callback(
    Query(query): Query<AuthRequest>,
    State(state): State<AppState>,
    session: tower_sessions::Session,
) -> HttpResult {
    let token = state
        .oauth_client
        .exchange_code(AuthorizationCode::new(query.code))
        .request_async(async_http_client)
        .await
        .map_err(|err| err.to_string())?;

    let client = reqwest::Client::new();
    let info: UserInfo = client
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

    let user = match model::User::by_email(&state.pool, &info.email).await? {
        Some(mut user) => {
            user.oauth_id = info.id;
            user.oauth_provider = provider.to_string();
            user.save(&state.pool).await?
        }
        None => {
            let user = model::User {
                id: 0,
                email: info.email,
                name: info.name.unwrap_or_default(),
                oauth_provider: provider.to_string(),
                oauth_id: info.id,
            };
            user.save(&state.pool).await?
        }
    };

    session
        .insert("user_id", user.id)
        .await
        .map_err(|err| format!("Unable to insert session: {err}"))?;

    session
        .save()
        .await
        .map_err(|err| format!("Unable to save session: {err}"))?;

    Ok(axum::response::Redirect::to("/dashboard").into_response())
}

pub async fn login(State(state): State<crate::AppState>) -> impl IntoResponse {
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
