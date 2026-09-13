pub use crate::{AppState, request_context::RequestContext};
pub use askama::Template;
pub use axum::extract::{Form, Path, Query, State};
use axum::http::StatusCode;
pub use axum::response::{Html, IntoResponse, Response};
pub use model::Pool;

pub type HttpResult = Result<Response, ErrorTemplate>;

#[derive(Template)]
#[template(path = "error.html")]
pub struct ErrorTemplate {
    friendly: String,
    redirect: Option<String>,
    status: StatusCode,
}

impl From<askama::Error> for ErrorTemplate {
    fn from(err: askama::Error) -> Self {
        tracing::error!(friendly = err.to_string(), error = err.to_string());
        ErrorTemplate {
            friendly: err.to_string(),
            redirect: None,
            status: StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<model::Error> for ErrorTemplate {
    fn from(err: model::Error) -> Self {
        let friendly = "Database error. Please contact the administrator.".to_owned();
        tracing::error!(friendly, error = err.to_string());
        ErrorTemplate {
            friendly,
            redirect: None,
            status: StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<&str> for ErrorTemplate {
    fn from(err: &str) -> Self {
        tracing::error!(friendly = err.to_string(), error = err.to_string());
        ErrorTemplate {
            friendly: err.to_string(),
            redirect: None,
            status: StatusCode::BAD_REQUEST,
        }
    }
}

impl From<String> for ErrorTemplate {
    fn from(err: String) -> Self {
        tracing::error!(friendly = err.to_string(), error = err.to_string());
        ErrorTemplate {
            friendly: err.to_string(),
            redirect: None,
            status: StatusCode::BAD_REQUEST,
        }
    }
}

impl IntoResponse for ErrorTemplate {
    fn into_response(self) -> axum::response::Response {
        if let Some(url) = self.redirect.as_ref() {
            axum::response::Redirect::to(url).into_response()
        } else {
            match self.render().map(axum::response::Html) {
                Ok(html) => (self.status, html).into_response(),
                Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
            }
        }
    }
}

pub fn env_or(key: &str, fallback: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| fallback.to_string())
}

pub async fn get_current_user(
    pool: &Pool,
    session: &tower_sessions::Session,
) -> Result<model::User, String> {
    let Ok(Some(user_id)) = session.get::<i64>("user_id").await else {
        return Err("No session".to_string())?;
    };

    match model::User::load(pool, user_id).await {
        Ok(Some(user)) => Ok(user),
        Ok(None) => Err("User not found".to_string()),
        Err(err) => Err(err.to_string()),
    }
}

pub async fn get_current_membership(
    pool: &Pool,
    session: &tower_sessions::Session,
) -> Result<(model::User, model::HouseholdMembership), String> {
    let user = get_current_user(pool, session).await?;
    let membership = model::HouseholdMembership::for_user(pool, user.id)
        .await
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "User is not a member of a household".to_string())?;
    Ok((user, membership))
}

pub fn is_manager(membership: &model::HouseholdMembership) -> bool {
    matches!(membership.role, model::Role::Manager)
}
