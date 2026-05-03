mod accounts;
mod auth;
mod categories;
mod home;
mod import_rules;

#[rustfmt::skip]
pub fn routes(state: super::AppState) -> axum::Router {
    axum::Router::new()
    .route("/", axum::routing::get(home::home))
    .route("/auth/callback", axum::routing::get(auth::callback))
    .route("/auth/login", axum::routing::get(auth::login))
    .route("/auth/logout", axum::routing::get(auth::logout))
    .route("/accounts", axum::routing::get(accounts::list))
    .route("/accounts/:id", axum::routing::get(accounts::edit).post(accounts::save))
    .route("/categories", axum::routing::get(categories::list))
    .route("/categories/:id", axum::routing::get(categories::edit).post(categories::save))
    .route("/import_rules", axum::routing::get(import_rules::list))
    .route("/import_rules/:id", axum::routing::get(import_rules::edit).post(import_rules::save))
    .with_state(state)
}
