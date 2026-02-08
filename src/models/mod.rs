use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub email: String,
    pub name: String,
    pub oauth_provider: String,
    pub oauth_id: String,
    pub created_at: String,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Account {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Category {
    pub id: i64,
    pub user_id: i64,
    pub name: String,
    pub color: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Transaction {
    pub id: i64,
    pub user_id: i64,
    pub category_id: Option<i64>,
    pub account_id: Option<i64>,
    pub amount: f64,
    pub description: String,
    pub transaction_date: String,
    #[serde(rename = "type")]
    pub transaction_type: String,
    pub account: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct NewCategory {
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct NewTransaction {
    pub category_id: Option<i64>,
    pub account_id: Option<i64>,
    pub amount: f64,
    pub description: String,
    pub transaction_date: String,
    #[serde(rename = "type")]
    pub transaction_type: String,
    pub account: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct NewAccount {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct TransactionWithCategory {
    pub id: i64,
    pub amount: f64,
    pub description: String,
    pub transaction_date: String,
    pub transaction_type: String,
    pub account: Option<String>,
    pub account_id: Option<i64>,
    pub account_name: Option<String>,
    pub category_name: Option<String>,
    pub category_color: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BudgetSummary {
    pub total_income: f64,
    pub total_expenses: f64,
    pub balance: f64,
}

#[derive(Debug, Serialize)]
pub struct PaginationInfo {
    pub current_page: i64,
    pub total_pages: i64,
    pub per_page: i64,
    pub total_items: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CsvUploadSession {
    pub file_id: String,
    pub file_path: String,
    pub headers: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ColumnMapping {
    pub file_id: String,
    pub date_column: String,
    pub amount_column: String,
    pub description_column: String,
    pub type_column: Option<String>,
    pub type_fixed_value: Option<String>,
    pub account_column: Option<String>,
    pub account_fixed_value: Option<String>,
    pub category_column: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ImportResult {
    pub total_rows: usize,
    pub successful: usize,
    pub failed: usize,
    pub errors: Vec<ImportError>,
    pub categories_created: Vec<String>,
    pub accounts_created: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ImportError {
    pub row_number: usize,
    pub row_data: String,
    pub error: String,
}
