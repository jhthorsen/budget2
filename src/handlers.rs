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

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct PaginationParams {
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_per_page")]
    pub per_page: i64,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct TransactionFilters {
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_per_page")]
    pub per_page: i64,
    #[serde(default, deserialize_with = "empty_string_as_none")]
    pub search: Option<String>,
    #[serde(default, deserialize_with = "empty_string_as_none")]
    pub transaction_type: Option<String>,
    #[serde(default, deserialize_with = "empty_string_as_none")]
    pub category_id: Option<String>,
    #[serde(default, deserialize_with = "empty_string_as_none_i64")]
    pub account_id: Option<i64>,
    #[serde(default, deserialize_with = "empty_string_as_none")]
    pub account: Option<String>,
    #[serde(default, deserialize_with = "empty_string_as_none")]
    pub date_from: Option<String>,
    #[serde(default, deserialize_with = "empty_string_as_none")]
    pub date_to: Option<String>,
}

fn default_page() -> i64 {
    1
}

fn default_per_page() -> i64 {
    20
}

fn empty_string_as_none<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    if s.trim().is_empty() {
        Ok(None)
    } else {
        Ok(Some(s))
    }
}

fn empty_string_as_none_i64<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    if s.trim().is_empty() {
        Ok(None)
    } else {
        s.parse::<i64>().map(Some).map_err(serde::de::Error::custom)
    }
}
