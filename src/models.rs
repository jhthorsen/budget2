mod accounts;
pub mod auth;
mod categories;
mod chart_data;
mod import_rules;
mod transactions;

pub use accounts::*;
pub use categories::*;
pub use chart_data::*;
pub use import_rules::*;
pub use transactions::*;

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

type Pool = sqlx::SqlitePool;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub email: String,
    pub name: String,
    pub oauth_provider: String,
    pub oauth_id: String,
}
