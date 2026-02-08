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
    pub amount: f64,
    pub description: String,
    pub transaction_date: String,
    #[serde(rename = "type")]
    pub transaction_type: String,
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
    pub amount: f64,
    pub description: String,
    pub transaction_date: String,
    #[serde(rename = "type")]
    pub transaction_type: String,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct TransactionWithCategory {
    pub id: i64,
    pub amount: f64,
    pub description: String,
    pub transaction_date: String,
    pub transaction_type: String,
    pub category_name: Option<String>,
    pub category_color: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BudgetSummary {
    pub total_income: f64,
    pub total_expenses: f64,
    pub balance: f64,
}
