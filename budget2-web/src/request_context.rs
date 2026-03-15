use rand::Rng;

#[derive(Debug, Clone)]
pub struct RequestContext {
    pub csr: bool,
    pub nonce: String,
    pub fluid: bool,
}

impl RequestContext {
    fn generate_nonce() -> String {
        rand::rng()
            .sample_iter(&rand::distr::Alphanumeric)
            .take(16)
            .map(char::from)
            .collect()
    }

    fn extract_nonce(headers: &axum::http::HeaderMap) -> String {
        headers
            .get("x-nonce")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .unwrap_or_else(Self::generate_nonce)
    }
}

#[axum::async_trait]
impl<S> axum::extract::FromRequestParts<S> for RequestContext
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        let nonce = Self::extract_nonce(&parts.headers);
        let csr = parts.headers.get("x-nonce").is_some();
        let fluid = false;

        Ok(RequestContext { csr, fluid, nonce })
    }
}
