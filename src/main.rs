mod auth;
mod handlers;
mod models;
mod filters;
mod request_context;

use anyhow::Result;
use auth::{AppState, create_oauth_client};
use axum::{
    routing::{get, post},
    Router,
};
use sqlx::sqlite::SqlitePoolOptions;
use tower_http::trace::TraceLayer;
use tower_sessions::{Expiry, SessionManagerLayer};
use tower_sessions_sqlx_store::SqliteStore;
use tower_sessions::cookie::SameSite;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "budget_app=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:budget.db".to_string());

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    let session_store = SqliteStore::new(pool.clone());
    session_store.migrate().await?;

    let session_layer = SessionManagerLayer::new(session_store)
        .with_name("budget_session")
        .with_same_site(SameSite::Lax)
        .with_secure(false) // Set to true in production with HTTPS
        .with_expiry(Expiry::OnInactivity(time::Duration::days(7)));

    let (oauth_client, userinfo_url) = create_oauth_client().await?;

    let state = AppState {
        pool: pool.clone(),
        oauth_client,
        userinfo_url,
    };

    let app = Router::new()
        .route("/", get(handlers::index::index_handler))
        .route("/dashboard", get(handlers::dashboard::dashboard_handler))
        .route("/login", get(handlers::login::login_handler))
        .route("/logout", get(handlers::logout::logout_handler))
        .route("/auth/callback", get(handlers::callback::auth_callback))
        .route("/transactions", post(handlers::transactions::create_transaction_handler))
        .route("/categories", post(handlers::categories::create_category_handler))
        .route("/accounts", post(handlers::accounts::create_account_handler))
        .route("/accounts/toggle-ownership", post(handlers::accounts::toggle_account_ownership_handler))
        .route("/import", get(handlers::csv::csv_upload_page))
        .route("/import/upload", post(handlers::csv::csv_upload_handler))
        .route("/import/process", post(handlers::csv::csv_import_handler))
        .route("/rules", get(handlers::rules::rules_list))
        .route("/rules/new", get(handlers::rules::rules_new_page).post(handlers::rules::rules_create))
        .route("/rules/:id/edit", get(handlers::rules::rules_edit_page).post(handlers::rules::rules_update))
        .route("/rules/:id/delete", post(handlers::rules::rules_delete))
        .route("/rules/:id/apply", post(handlers::rules::rules_apply))
        .layer(session_layer)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await?;
    
    tracing::info!("Listening on http://{}", listener.local_addr()?);
    
    axum::serve(listener, app).await?;

    Ok(())
}
