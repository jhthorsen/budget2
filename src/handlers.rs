use askama::Template;

pub mod accounts;
pub mod auth;
pub mod categories;
pub mod dashboard;
pub mod import;
// pub mod rules;
// pub mod settings;
pub mod static_files;
pub mod transactions;

type HttpResult = Result<axum::response::Response, ErrorTemplate>;

#[derive(Template)]
#[template(path = "error.html")]
pub struct ErrorTemplate {
    friendly: String,
    redirect: Option<String>,
}

impl axum::response::IntoResponse for ErrorTemplate {
    fn into_response(self) -> axum::response::Response {
        if let Some(url) = self.redirect.as_ref() {
            axum::response::Redirect::to(url).into_response()
        } else {
            self.render()
                .map(axum::response::Html)
                .map_err(|err| {
                    tracing::error!(category = "template", error = err.to_string());
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR
                })
                .into_response()
        }
    }
}

impl From<askama::Error> for ErrorTemplate {
    fn from(err: askama::Error) -> Self {
        tracing::error!(friendly = err.to_string(), error = err.to_string());
        ErrorTemplate {
            friendly: err.to_string(),
            redirect: None,
        }
    }
}

impl From<sqlx::Error> for ErrorTemplate {
    fn from(err: sqlx::Error) -> Self {
        let friendly = "Unable to communicate with the databsae".to_owned();
        tracing::error!(friendly, error = err.to_string());
        ErrorTemplate {
            friendly,
            redirect: None,
        }
    }
}

impl From<&str> for ErrorTemplate {
    fn from(err: &str) -> Self {
        tracing::error!(friendly = err.to_string(), error = err.to_string());
        ErrorTemplate {
            friendly: err.to_string(),
            redirect: None,
        }
    }
}

impl From<String> for ErrorTemplate {
    fn from(err: String) -> Self {
        tracing::error!(friendly = err.to_string(), error = err.to_string());
        ErrorTemplate {
            friendly: err.to_string(),
            redirect: None,
        }
    }
}
