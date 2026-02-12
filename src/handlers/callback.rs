use crate::models::auth::{OAuthUserInfo, SESSION_USER_KEY};
use crate::models::*;
use crate::AppState;
use axum::extract::{Query, State};
use axum::response::IntoResponse;
use oauth2::{reqwest::async_http_client, AuthorizationCode, TokenResponse};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct AuthRequest {
    code: String,
    // TODO Need to verify state=xyz
    #[allow(dead_code)]
    state: String,
}

pub async fn auth_callback(
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

    let user = get_or_create_user(&state.pool, &user_info).await?;

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

async fn get_or_create_user(
    pool: &sqlx::SqlitePool,
    user_info: &OAuthUserInfo,
) -> Result<User, axum::response::Response> {
    let provider = "default";
    let name = user_info.name.as_deref().unwrap_or(&user_info.email);

    let user =
        sqlx::query_as::<_, User>("SELECT * FROM users WHERE oauth_provider = ? AND oauth_id = ?")
            .bind(provider)
            .bind(&user_info.id)
            .fetch_optional(pool)
            .await
            .map_err(|err| super::db_error(err, "Database error"))?;

    if let Some(user) = user {
        return Ok(user);
    }

    let result = sqlx::query(
        "INSERT INTO users (email, name, oauth_provider, oauth_id) VALUES (?, ?, ?, ?)",
    )
    .bind(&user_info.email)
    .bind(name)
    .bind(provider)
    .bind(&user_info.id)
    .execute(pool)
    .await
    .map_err(|err| super::db_error(err, "Database error"))?;

    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
        .bind(result.last_insert_rowid())
        .fetch_one(pool)
        .await
        .map_err(|err| super::db_error(err, "Database error"))?;

    Ok(user)
}
