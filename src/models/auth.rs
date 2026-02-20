use super::User;
use oauth2::{AuthUrl, ClientId, ClientSecret, RedirectUrl, TokenUrl, basic::BasicClient};
use serde::Deserialize;

pub(crate) const SESSION_USER_KEY: &str = "user_id";

#[derive(Debug)]
pub enum OauthError {
    Environment(String),
    Ouath(String),
    Reqwest(String),
}

impl From<oauth2::url::ParseError> for OauthError {
    fn from(err: oauth2::url::ParseError) -> Self {
        Self::Ouath(err.to_string())
    }
}

impl From<reqwest::Error> for OauthError {
    fn from(err: reqwest::Error) -> Self {
        Self::Reqwest(err.to_string())
    }
}

impl From<std::env::VarError> for OauthError {
    fn from(err: std::env::VarError) -> Self {
        Self::Environment(err.to_string())
    }
}

impl std::fmt::Display for OauthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OauthError::Environment(reason) => write!(f, "{reason}"),
            OauthError::Ouath(reason) => write!(f, "{reason}"),
            OauthError::Reqwest(reason) => write!(f, "{reason}"),
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct OAuthUserInfo {
    pub email: String,
    pub name: Option<String>,
    #[serde(alias = "sub")]
    pub id: String,
}

impl OAuthUserInfo {
    pub async fn get_or_create_user(
        &self,
        pool: &sqlx::SqlitePool,
        provider: &str,
    ) -> Result<User, sqlx::Error> {
        let user = sqlx::query_as!(
            User,
            r#"select id as 'id!', email, name, oauth_provider, oauth_id
            from users
            where oauth_provider = ? and oauth_id = ?"#,
            provider,
            self.id,
        )
        .fetch_optional(pool)
        .await?;

        if let Some(user) = user {
            return Ok(user);
        }

        let name = self.name.as_deref().unwrap_or(&self.email);
        sqlx::query!(
            r#"insert or ignore into users (email, name, oauth_provider, oauth_id) values (?, ?, ?, ?)"#,
            self.email,
            name,
            provider,
            self.id,
        )
        .execute(pool)
        .await?;

        sqlx::query_as!(
            User,
            r#"select id as 'id!', email, name, oauth_provider, oauth_id from users where email = ?"#,
            self.email,
        )
        .fetch_one(pool)
        .await
    }
}

#[derive(Debug, Deserialize)]
struct OidcDiscovery {
    authorization_endpoint: String,
    token_endpoint: String,
    userinfo_endpoint: String,
}

pub async fn create_oauth_client() -> Result<(BasicClient, String), OauthError> {
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

pub async fn get_current_user(
    pool: &sqlx::SqlitePool,
    session: &tower_sessions::Session,
) -> Result<User, String> {
    let Ok(Some(user_id)) = session.get::<i64>(SESSION_USER_KEY).await else {
        return Err("No session".to_string())?;
    };

    sqlx::query_as!(
        User,
        r#"select id as 'id!', email, name, oauth_provider, oauth_id
        from users
        where id = ?"#,
        user_id
    )
    .fetch_optional(pool)
    .await
    .map_err(|err| err.to_string())?
    .ok_or("User not found".to_string())
}
