pub mod csv;
pub mod rules;
pub mod index;
pub mod dashboard;
pub mod transactions;
pub mod categories;
pub mod accounts;
pub mod login;
pub mod logout;
pub mod callback;
pub mod add_transaction;
pub mod account;

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize, Clone, Serialize)]
pub struct TransactionFilters {
    #[serde(default)]
    pub account_id: i64,
    #[serde(default)]
    pub account: String,
    #[serde(default)]
    pub category_id: i64,
    #[serde(default)]
    pub filtered: bool,
    #[serde(default)]
    pub month: String,
    #[serde(default)]
    pub page: i64,
    #[serde(default)]
    pub search: String,
    #[serde(default)]
    pub transaction_type: String,
}
