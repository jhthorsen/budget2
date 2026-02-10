use anyhow::Result;
use oauth2::{
    basic::BasicClient, AuthUrl, ClientId,
    ClientSecret, RedirectUrl, TokenUrl,
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

pub(crate) const SESSION_USER_KEY: &str = "user_id";

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub oauth_client: BasicClient,
    pub userinfo_url: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OAuthUserInfo {
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

pub async fn get_current_user(session: &Session, pool: &SqlitePool) -> Option<User> {
    let user_id: i64 = session.get(SESSION_USER_KEY).await.ok()??;

    sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .ok()?
}
