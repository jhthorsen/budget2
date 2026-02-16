use askama::Template;
use axum::response::IntoResponse;

pub mod accounts;
pub mod auth;
pub mod categories;
pub mod csv;
pub mod dashboard;
pub mod rules;
pub mod settings;
pub mod static_files;
pub mod transactions;

#[derive(Template)]
#[template(path = "error.html")]
struct ErrorTemplate {
    friendly: String,
}

fn db_error(err: sqlx::Error, friendly: &str) -> axum::response::Response {
    render_error(&err.to_string(), friendly)
}

fn render_error(err: &str, friendly: &str) -> axum::response::Response {
    let friendly = match friendly.is_empty() {
        true => err,
        false => friendly,
    };

    tracing::error!(category="render", friendly, error=err);

    let template = ErrorTemplate {
        friendly: friendly.to_owned(),
    };

    match template.render().map(axum::response::Html) {
        Ok(html) => html.into_response(),
        Err(err) => {
            tracing::error!(category="template", error=err.to_string());
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Unable to render error template".to_owned(),
            )
                .into_response()
        }
    }
}

fn session_error(err: Option<tower_sessions::session::Error>) -> axum::response::Response {
    match err {
        Some(err) => render_error(&err.to_string(), "Request does not match active session"),
        None => render_error("No active session", "No active session"),
    }
}

fn template_error(err: askama::Error) -> axum::response::Response {
    render_error(&err.to_string(), "Unable to render template")
}
