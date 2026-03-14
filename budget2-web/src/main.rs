mod handlers;
mod helpers;

use helpers::env_or;
use tower_sessions_sqlx_store::SqliteStore;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Clone)]
pub struct AppState {
    pub pool: model::Pool,
}

fn bind_address() -> String {
    let host = env_or("HOST", "127.0.0.1");
    let port = env_or("PORT", "3000")
        .parse::<u16>()
        .expect("PORT= must be an integer");

    format!("{host}:{port}")
}

fn setup_tracing() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "budget2_web=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    setup_tracing();

    let pool = model::build_pool(&env_or("DATABASE_URL", "sqlite:local/budget2.db"), true)
        .await
        .expect("Must be able to connect to database");

    let session_store = SqliteStore::new(pool.clone());
    session_store.migrate().await.expect("A session store");

    #[rustfmt::skip]
    let session_layer = tower_sessions::SessionManagerLayer::new(session_store)
        .with_name("budget_session")
        .with_same_site(tower_sessions::cookie::SameSite::Lax)
        .with_secure(matches!(env_or("SECURE_SESSION", "true").as_str(), "1" | "true"))
        .with_expiry(tower_sessions::Expiry::OnInactivity(time::Duration::days(7)));

    let state = AppState { pool };
    let app = axum::Router::new()
        .nest("/", handlers::routes(state))
        .layer(session_layer)
        .layer(tower_http::trace::TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(&bind_address())
        .await
        .expect("To listen to port");

    tracing::info!("Availablle at http://{}", listener.local_addr().unwrap());
    axum::serve(listener, app)
        .await
        .expect("To serve at address");
}
