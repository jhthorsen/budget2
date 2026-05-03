mod accounts;
mod categories;
mod import_rules;
mod user;

pub use accounts::*;
pub use categories::*;
pub use import_rules::*;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
pub use user::User;

pub type DbResult<T> = Result<T, sqlx::Error>;
pub type Error = sqlx::Error;
pub type Pool = sqlx::SqlitePool;

pub async fn build_pool(database_url: &str, migrate: bool) -> DbResult<Pool> {
    let options =
        sqlx::sqlite::SqliteConnectOptions::from_str(database_url)?.create_if_missing(true);

    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    if migrate {
        sqlx::migrate!("./migrations").run(&pool).await?;
    }

    Ok(pool)
}

fn invalid(reason: &str) -> Result<(), sqlx::Error> {
    Err(sqlx::Error::Protocol(reason.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_build_pool_in_memory() {
        let database_url = "sqlite::memory:";
        let result = build_pool(database_url, false).await;
        assert!(result.is_ok(), "Expected Ok(Pool), got {:?}", result);
    }
}
