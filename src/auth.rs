use anyhow::Result;
use axum::{
    extract::{Query, State},
    response::{IntoResponse, Redirect},
};
use oauth2::{
    basic::BasicClient, reqwest::async_http_client, AuthUrl, AuthorizationCode, ClientId,
    ClientSecret, CsrfToken, RedirectUrl, Scope, TokenResponse, TokenUrl,
};
use serde::Deserialize;
use sqlx::SqlitePool;
use tower_sessions::Session;

use crate::models::User;

#[derive(Debug, Deserialize)]
struct OidcDiscovery {
    authorization_endpoint: String,
    token_endpoint: String,
    userinfo_endpoint: String,
}

const SESSION_USER_KEY: &str = "user_id";

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub oauth_client: BasicClient,
    pub userinfo_url: String,
}

#[derive(Debug, Deserialize)]
pub struct OAuthUserInfo {
    pub email: String,
    pub name: Option<String>,
    #[serde(alias = "sub")]
    pub id: String,
}

pub async fn create_oauth_client() -> Result<(BasicClient, String)> {
    let client_id = std::env::var("OAUTH_CLIENT_ID")?;
    let client_secret = std::env::var("OAUTH_CLIENT_SECRET")?;
    let redirect_url = std::env::var("OAUTH_REDIRECT_URL")?;
    let discovery_url = std::env::var("OAUTH_DISCOVERY_URL")
        .unwrap_or_else(|_| "https://accounts.google.com/.well-known/openid-configuration".to_string());

    // Fetch OIDC discovery document
    let client = reqwest::Client::new();
    let discovery: OidcDiscovery = client
        .get(&discovery_url)
        .send()
        .await?
        .json()
        .await?;

    let oauth_client = BasicClient::new(
        ClientId::new(client_id),
        Some(ClientSecret::new(client_secret)),
        AuthUrl::new(discovery.authorization_endpoint)?,
        Some(TokenUrl::new(discovery.token_endpoint)?),
    )
    .set_redirect_uri(RedirectUrl::new(redirect_url)?);

    Ok((oauth_client, discovery.userinfo_endpoint))
}

pub async fn login_handler(State(state): State<AppState>) -> impl IntoResponse {
    let (auth_url, _csrf_token) = state
        .oauth_client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("openid".to_string()))
        .add_scope(Scope::new("email".to_string()))
        .add_scope(Scope::new("profile".to_string()))
        .url();

    Redirect::to(auth_url.as_str())
}

#[derive(Debug, Deserialize)]
pub struct AuthRequest {
    code: String,
    #[allow(dead_code)]
    state: String,
}

pub async fn auth_callback(
    Query(query): Query<AuthRequest>,
    State(state): State<AppState>,
    session: Session,
) -> Result<impl IntoResponse, String> {
    let token = state
        .oauth_client
        .exchange_code(AuthorizationCode::new(query.code))
        .request_async(async_http_client)
        .await
        .map_err(|e| format!("Failed to get token: {}", e))?;

    let client = reqwest::Client::new();
    let user_info: OAuthUserInfo = client
        .get(&state.userinfo_url)
        .bearer_auth(token.access_token().secret())
        .send()
        .await
        .map_err(|e| format!("Failed to fetch user info: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse user info: {}", e))?;

    let user = get_or_create_user(&state.pool, &user_info)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

    session
        .insert(SESSION_USER_KEY, user.id)
        .await
        .map_err(|e| format!("Session error: {}", e))?;

    session
        .save()
        .await
        .map_err(|e| format!("Session save error: {}", e))?;

    Ok(Redirect::to("/dashboard"))
}

async fn get_or_create_user(pool: &SqlitePool, user_info: &OAuthUserInfo) -> Result<User> {
    let provider = "google";
    let name = user_info.name.as_deref().unwrap_or(&user_info.email);

    let user = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE oauth_provider = ? AND oauth_id = ?",
    )
    .bind(provider)
    .bind(&user_info.id)
    .fetch_optional(pool)
    .await?;

    if let Some(user) = user {
        Ok(user)
    } else {
        let result = sqlx::query(
            "INSERT INTO users (email, name, oauth_provider, oauth_id) VALUES (?, ?, ?, ?)",
        )
        .bind(&user_info.email)
        .bind(name)
        .bind(provider)
        .bind(&user_info.id)
        .execute(pool)
        .await?;

        let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
            .bind(result.last_insert_rowid())
            .fetch_one(pool)
            .await?;

        Ok(user)
    }
}

pub async fn logout_handler(session: Session) -> impl IntoResponse {
    session.delete().await.ok();
    Redirect::to("/")
}

pub async fn get_current_user(session: &Session, pool: &SqlitePool) -> Option<User> {
    let user_id: i64 = session.get(SESSION_USER_KEY).await.ok()??;

    sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .ok()?
}
