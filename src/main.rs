mod auth;
mod handlers;
mod models;
mod filters;

use anyhow::Result;
use auth::{auth_callback, create_oauth_client, login_handler, logout_handler, AppState};
use axum::{
    routing::{get, post},
    Router,
};
use handlers::{
    create_category_handler, create_transaction_handler, create_account_handler, 
    toggle_account_ownership_handler, dashboard_handler, index_handler,
    csv::{csv_upload_page, csv_upload_handler, csv_import_handler},
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

    let oauth_client = create_oauth_client()?;

    let state = AppState {
        pool: pool.clone(),
        oauth_client,
    };

    let app = Router::new()
        .route("/", get(index_handler))
        .route("/dashboard", get(dashboard_handler))
        .route("/login", get(login_handler))
        .route("/logout", get(logout_handler))
        .route("/auth/callback", get(auth_callback))
        .route("/transactions", post(create_transaction_handler))
        .route("/categories", post(create_category_handler))
        .route("/accounts", post(create_account_handler))
        .route("/accounts/toggle-ownership", post(toggle_account_ownership_handler))
        .route("/import", get(csv_upload_page))
        .route("/import/upload", post(csv_upload_handler))
        .route("/import/process", post(csv_import_handler))
        .layer(session_layer)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await?;
    
    tracing::info!("Listening on http://{}", listener.local_addr()?);
    
    axum::serve(listener, app).await?;

    Ok(())
}
