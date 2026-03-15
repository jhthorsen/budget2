mod accounts;
mod home;

pub fn routes(state: super::AppState) -> axum::Router {
    axum::Router::new()
        .route("/accounts", axum::routing::get(accounts::list))
        .route(
            "/accounts/:id",
            axum::routing::get(accounts::edit).post(accounts::save),
        )
        .route("/", axum::routing::get(home::home))
        .with_state(state)
}
