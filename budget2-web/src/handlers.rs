mod accounts;
mod auth;
mod home;

pub fn routes(state: super::AppState) -> axum::Router {
    axum::Router::new()
        .route("/accounts", axum::routing::get(accounts::list))
        .route(
            "/accounts/:id",
            axum::routing::get(accounts::edit).post(accounts::save),
        )
        .route("/", axum::routing::get(home::home))
        .route("/auth/callback", axum::routing::get(auth::callback))
        .route("/auth/login", axum::routing::get(auth::login))
        .route("/auth/logout", axum::routing::get(auth::logout))
        .with_state(state)
}
