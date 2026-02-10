use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{request::Parts, HeaderMap},
};
use rand::Rng;
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct RequestContext {
    pub csr: bool,
    pub nonce: String,
}

#[derive(Debug, Deserialize)]
struct CsrQuery {
    #[serde(default)]
    csr: Option<String>,
}

impl RequestContext {
    fn generate_nonce() -> String {
        const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
        let mut rng = rand::thread_rng();
        (0..16)
            .map(|_| {
                let idx = rng.gen_range(0..CHARSET.len());
                CHARSET[idx] as char
            })
            .collect()
    }

    fn extract_nonce(headers: &HeaderMap) -> String {
        headers
            .get("x-nonce")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .unwrap_or_else(Self::generate_nonce)
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for RequestContext
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let nonce = Self::extract_nonce(&parts.headers);
        let csr = parts.headers.get("x-nonce").is_some();

        Ok(RequestContext { csr, nonce })
    }
}
