use axum::response::{IntoResponse, Redirect};
use tower_sessions::Session;

pub async fn logout_handler(session: Session) -> impl IntoResponse {
    session.delete().await.ok();
    Redirect::to("/")
}
