mod filters;
mod handlers;
mod models;
mod request_context;

use axum::routing::{get, post};
use tower_sessions_sqlx_store::SqliteStore;

type HttpResult = Result<axum::response::Response, axum::response::Response>;

#[derive(Clone)]
pub struct AppState {
    pub pool: sqlx::SqlitePool,
    pub oauth_client: oauth2::basic::BasicClient,
    pub userinfo_url: String,
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    env_logger::Builder::from_env(env_logger::Env::default()).init();

    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:budget.db".to_string());
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("To connect to database");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("To migrate the database");

    let secure = matches!(
        std::env::var("SECURE_SESSION").unwrap_or_default().as_str(),
        "1" | "true"
    );
    let session_store = SqliteStore::new(pool.clone());
    session_store
        .migrate()
        .await
        .expect("To migrate the session store");

    let session_layer = tower_sessions::SessionManagerLayer::new(session_store)
        .with_name("budget_session")
        .with_same_site(tower_sessions::cookie::SameSite::Lax)
        .with_secure(secure)
        .with_expiry(tower_sessions::Expiry::OnInactivity(time::Duration::days(
            7,
        )));

    let (oauth_client, userinfo_url) = match models::auth::create_oauth_client().await {
        Ok((oauth_client, userinfo_url)) => (oauth_client, userinfo_url),
        Err(err) => panic!("Unable to create oauth client: {err}"),
    };

    let state = AppState {
        pool: pool.clone(),
        oauth_client,
        userinfo_url,
    };

    let app = axum::Router::new()
        .route("/", get(handlers::auth::index_handler))
        .route("/static/:file", get(handlers::static_files::get))
        .route("/auth/callback", get(handlers::callback::auth_callback))
        .route("/login", get(handlers::auth::login_handler))
        .route("/logout", get(handlers::auth::logout_handler))
        .route(
            "/accounts",
            post(handlers::accounts::create_account_handler),
        )
        .route(
            "/accounts/toggle-ownership",
            post(handlers::accounts::toggle_account_ownership_handler),
        )
        .route("/add", get(handlers::transactions::add_transaction_page))
        .route(
            "/categories",
            post(handlers::categories::create_category_handler),
        )
        .route("/dashboard", get(handlers::dashboard::dashboard_handler))
        .route("/import", get(handlers::csv::csv_upload_page))
        .route("/import/process", post(handlers::csv::csv_import_handler))
        .route("/import/upload", post(handlers::csv::csv_upload_handler))
        .route("/rules", get(handlers::rules::rules_list))
        .route("/rules/:id/apply", post(handlers::rules::rules_apply))
        .route("/rules/:id/delete", post(handlers::rules::rules_delete))
        .route(
            "/rules/:id/edit",
            get(handlers::rules::rules_edit_page).post(handlers::rules::rules_update),
        )
        .route(
            "/rules/new",
            get(handlers::rules::rules_new_page).post(handlers::rules::rules_create),
        )
        .route("/settings", get(handlers::settings::settings_page))
        .route(
            "/transactions",
            post(handlers::transactions::create_transaction_handler),
        )
        .layer(session_layer)
        .with_state(state);

    let port = std::env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse::<u16>()
        .expect("PORT to be a valid u16");

    let listener = tokio::net::TcpListener::bind(&format!("127.0.0.1:{port}"))
        .await
        .expect("To listen to port");

    log::info!(port; "http://{}", listener.local_addr().unwrap());

    axum::serve(listener, app)
        .await
        .expect("To serve at address");
}
