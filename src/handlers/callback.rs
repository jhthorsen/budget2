use anyhow::Result;
use axum::{
    extract::{Query, State},
    response::{IntoResponse, Redirect},
};
use oauth2::{reqwest::async_http_client, AuthorizationCode, TokenResponse};
use serde::Deserialize;
use sqlx::SqlitePool;
use tower_sessions::Session;

use crate::auth::{AppState, OAuthUserInfo, SESSION_USER_KEY};
use crate::models::User;

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
