mod handlers;
mod helpers;
#[cfg(not(feature = "offline"))]
mod oidc;
mod request_context;

use axum::routing::get;
use helpers::env_or;
use tower_sessions_sqlx_store::SqliteStore;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Clone)]
pub struct AppState {
    #[cfg(not(feature = "offline"))]
    pub oauth_client: oauth2::basic::BasicClient,
    pub pool: model::Pool,
    #[cfg(not(feature = "offline"))]
    pub userinfo_url: String,
}

async fn listen(app: axum::Router) -> Result<(), std::io::Error> {
    let host = env_or("HOST", "127.0.0.1");
    let port = env_or("PORT", "3000")
        .parse::<u16>()
        .expect("PORT= must be an integer");

    tracing::info!(%host, port, "Starting web server");
    let listener = tokio::net::TcpListener::bind(&format!("{host}:{port}"))
        .await
        .expect("To listen to port");

    tracing::info!(address = %listener.local_addr().unwrap(), "Web server ready");
    axum::serve(listener, app).await
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
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "Starting budget2-web");

    let db = env_or("DATABASE_URL", "sqlite:local/budget2.db");
    tracing::info!("Connecting to database {db}");
    let skip_migrations = matches!(
        env_or("SKIP_MIGRATIONS", "false")
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true" | "yes"
    );
    if skip_migrations {
        tracing::warn!("Skipping database migrations because SKIP_MIGRATIONS is enabled");
    }
    let pool = model::build_pool(&db, !skip_migrations)
        .await
        .expect("Must be able to connect to database");
    tracing::info!(migrations = !skip_migrations, "Database connected");

    let session_store = SqliteStore::new(pool.clone());
    tracing::info!("Migrating session store");
    session_store.migrate().await.expect("A session store");
    tracing::info!("Session store ready");

    #[rustfmt::skip]
    let session_layer = tower_sessions::SessionManagerLayer::new(session_store)
        .with_name("budget_session")
        .with_same_site(tower_sessions::cookie::SameSite::Lax)
        .with_secure(matches!(env_or("SECURE_SESSION", "true").as_str(), "1" | "true"))
        .with_expiry(tower_sessions::Expiry::OnInactivity(time::Duration::days(7)));

    #[cfg(not(feature = "offline"))]
    tracing::info!("Configuring OIDC client");
    #[cfg(not(feature = "offline"))]
    let (oauth_client, userinfo_url) = oidc::build_client()
        .await
        .unwrap_or_else(|err| panic!("Failed to create OAuth client: {err}"));
    #[cfg(not(feature = "offline"))]
    tracing::info!("OIDC client ready");
    #[cfg(feature = "offline")]
    tracing::info!("Running with offline authentication");
    let state = AppState {
        #[cfg(not(feature = "offline"))]
        oauth_client,
        pool,
        #[cfg(not(feature = "offline"))]
        userinfo_url,
    };

    let app = axum::Router::new()
        .route("/static/:file", get(static_files::get))
        .nest("/", handlers::routes(state))
        .layer(session_layer)
        .layer(tower_http::trace::TraceLayer::new_for_http());

    tracing::info!("Application routes configured");
    listen(app)
        .await
        .expect("Should be able to start the server");
}
