use oauth2::{AuthUrl, ClientId, ClientSecret, RedirectUrl, TokenUrl, basic::BasicClient};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct OidcDiscovery {
    authorization_endpoint: String,
    token_endpoint: String,
    userinfo_endpoint: String,
}

pub async fn build_client() -> Result<(BasicClient, String), String> {
    let client_id = std::env::var("OIDC_CLIENT_ID")
        .map_err(|_| "OIDC_CLIENT_ID environment variable is required".to_string())?;
    let client_secret = std::env::var("OIDC_CLIENT_SECRET")
        .map_err(|_| "OIDC_CLIENT_SECRET environment variable is required".to_string())?;
    let discovery_url = std::env::var("OIDC_DISCOVERY_URL")
        .map_err(|_| "OIDC_DISCOVERY_URL environment variable is required".to_string())?;
    let redirect_url = RedirectUrl::new(
        std::env::var("OIDC_REDIRECT_URL")
            .map_err(|_| "OIDC_REDIRECT_URL environment variable is required".to_string())?,
    )
    .map_err(|err| format!("Invalid OIDC_REDIRECT_URL {err}"))?;

    let client = reqwest::Client::new();
    let discovery: OidcDiscovery = client
        .get(&discovery_url)
        .send()
        .await
        .map_err(|err| format!("OIDC {discovery_url} failed: {err}"))?
        .json()
        .await
        .map_err(|err| format!("OIDC {discovery_url} failed: {err}"))?;

    let auth_url = AuthUrl::new(discovery.authorization_endpoint)
        .map_err(|err| format!("OIDC got invalid authorization_endpoint: {err}"))?;
    let token_url = TokenUrl::new(discovery.token_endpoint)
        .map_err(|err| format!("OIDC got invalid token_endpoint: {err}"))?;

    let oauth_client = BasicClient::new(
        ClientId::new(client_id),
        Some(ClientSecret::new(client_secret)),
        auth_url,
        Some(token_url),
    )
    .set_redirect_uri(redirect_url);

    Ok((oauth_client, discovery.userinfo_endpoint))
}
