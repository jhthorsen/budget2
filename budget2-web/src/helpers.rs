pub use axum::response::IntoResponse;
pub use axum::response::Response;

pub fn env_or(key: &str, fallback: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| fallback.to_string())
}
