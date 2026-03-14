mod home;

pub fn routes(state: super::AppState) -> axum::Router {
    axum::Router::new()
        .route("/", axum::routing::get(home::home))
        .with_state(state)
}
