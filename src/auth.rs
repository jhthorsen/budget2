use crate::models::User;
use anyhow::Result;
use oauth2::basic::BasicClient;
use oauth2::{AuthUrl, ClientId, ClientSecret, RedirectUrl, TokenUrl};
use serde::Deserialize;
use sqlx::SqlitePool;
use tower_sessions::Session;

pub const SESSION_USER_KEY: &str = "user_id";

#[derive(Debug, Deserialize)]
struct OidcDiscovery {
    authorization_endpoint: String,
    token_endpoint: String,
    userinfo_endpoint: String,
}

#[derive(Debug, Deserialize)]
pub struct OAuthUserInfo {
    #[serde(alias = "sub")]
    pub id: String,
    pub email: String,
    pub name: Option<String>,
}

pub async fn create_oauth_client() -> Result<(BasicClient, String)> {
    let client_id = std::env::var("OAUTH_CLIENT_ID")?;
    let client_secret = std::env::var("OAUTH_CLIENT_SECRET")?;
    let redirect_url = std::env::var("OAUTH_REDIRECT_URL")?;
    let discovery_url = std::env::var("OAUTH_DISCOVERY_URL")?;

    let client = reqwest::Client::new();
    let discovery: OidcDiscovery = client.get(&discovery_url).send().await?.json().await?;

    let oauth_client = BasicClient::new(
        ClientId::new(client_id),
        Some(ClientSecret::new(client_secret)),
        AuthUrl::new(discovery.authorization_endpoint)?,
        Some(TokenUrl::new(discovery.token_endpoint)?),
    )
    .set_redirect_uri(RedirectUrl::new(redirect_url)?);

    Ok((oauth_client, discovery.userinfo_endpoint))
}

pub async fn get_current_user(session: &Session, pool: &SqlitePool) -> Option<User> {
    let id: i64 = session.get(SESSION_USER_KEY).await.ok()??;
    sqlx::query_as!(
        User,
        "SELECT id as 'id!', email, name, oauth_provider, oauth_id, created_at FROM users WHERE id = ?",
        id,
    )
    .fetch_optional(pool)
    .await.ok()?
}

pub async fn get_or_create_user(pool: &SqlitePool, user_info: &OAuthUserInfo) -> Result<User> {
    let provider = "default"; // TODO
    let user = sqlx::query_as!(
        User,
        "SELECT id as 'id!', email, name, oauth_provider, oauth_id, created_at FROM users WHERE oauth_provider = ? AND oauth_id = ?",
        provider,
        user_info.id
    )
    .fetch_optional(pool)
    .await?;

    if let Some(user) = user {
        return Ok(user);
    }

    let name = user_info.name.as_deref().unwrap_or(&user_info.email);
    let res = sqlx::query_as!(
        User,
        "INSERT INTO users (email, name, oauth_provider, oauth_id) VALUES (?, ?, ?, ?)",
        user_info.email,
        name,
        provider,
        user_info.id
    )
    .execute(pool)
    .await?;

    let id = res.last_insert_rowid();
    Ok(sqlx::query_as!(
        User,
        "SELECT id as 'id!', email, name, oauth_provider, oauth_id, created_at FROM users WHERE id = ?",
        id
    )
    .fetch_one(pool)
    .await?)
}
