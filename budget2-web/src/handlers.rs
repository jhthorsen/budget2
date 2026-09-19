mod accounts;
mod auth;
mod categories;
mod dashboard;
mod home;
mod household;
mod import;
mod import_rules;

#[rustfmt::skip]
pub fn routes(state: super::AppState) -> axum::Router {
    axum::Router::new()
    .route("/", axum::routing::get(home::get))
    .route("/dashboard", axum::routing::get(dashboard::get))
    .route("/dashboard/transactions", axum::routing::get(dashboard::transactions))
    .route("/import", axum::routing::get(import::upload_form))
    .route("/import/upload", axum::routing::post(import::upload_then_map_columns))
    .route("/import/process", axum::routing::post(import::import_uploaded))
    .route("/auth/callback", axum::routing::get(auth::callback))
    .route("/auth/login", axum::routing::get(auth::login))
    .route("/auth/logout", axum::routing::post(auth::logout))
    .route("/accounts", axum::routing::get(accounts::list))
    .route("/accounts/:id", axum::routing::get(accounts::edit).post(accounts::save))
    .route("/categories", axum::routing::get(categories::list))
    .route("/categories/:id", axum::routing::get(categories::edit).post(categories::save))
    .route("/import_rules", axum::routing::get(import_rules::list))
    .route("/import_rules/:id", axum::routing::get(import_rules::edit).post(import_rules::save))
    .route("/import_rules/:id/match", axum::routing::post(import_rules::match_uncategorized))
    .route("/import_rules/:id/delete", axum::routing::post(import_rules::delete))
    .route("/household", axum::routing::get(household::get))
    .route("/household/role", axum::routing::post(household::set_role))
    .with_state(state)
}
