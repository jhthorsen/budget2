use axum::{
    extract::State,
    response::{IntoResponse, Redirect},
};
use oauth2::{CsrfToken, Scope};

use crate::auth::AppState;

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
