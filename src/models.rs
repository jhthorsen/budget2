pub mod auth;

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Default, FromRow, Serialize, Deserialize)]
pub struct AccountWithOwnership {
    pub id: i64,
    #[serde(default)]
    pub description: Option<String>,
    pub is_mine: bool,
    #[serde(default)]
    pub name: String,
}

impl AccountWithOwnership {
    pub async fn create_for_user(
        &self,
        pool: &sqlx::SqlitePool,
        user_id: i64,
    ) -> Result<i64, sqlx::Error> {
        let account_id: i64 =
            sqlx::query_scalar!("select id as 'id!' from accounts where name = ?", self.name)
                .fetch_optional(pool)
                .await?
                .unwrap_or(
                    sqlx::query!(
                        "insert into accounts (name, description) values (?, ?)",
                        self.name,
                        self.description,
                    )
                    .execute(pool)
                    .await?
                    .last_insert_rowid(),
                );

        sqlx::query!(
            "insert or ignore into user_accounts (user_id, account_id) values (?, ?)",
            user_id,
            account_id
        )
        .execute(pool)
        .await?;

        Ok(account_id)
    }

    pub async fn ensure(
        pool: &sqlx::Pool<sqlx::Sqlite>,
        account_name: &str,
    ) -> Result<i64, sqlx::Error> {
        Ok(sqlx::query_scalar!(
            "select id as 'id!' from accounts where name = ?",
            account_name
        )
        .fetch_optional(pool)
        .await?
        .unwrap_or(
            sqlx::query!(
                "insert into accounts (name, description) values (?, '')",
                account_name,
            )
            .execute(pool)
            .await?
            .last_insert_rowid(),
        ))
    }

    pub async fn accounts_for_user(
        pool: &sqlx::SqlitePool,
        user_id: i64,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            Self,
            r#"select a.id as 'id!', a.name, a.description, ua.is_mine as 'is_mine:bool'
            from accounts a
            inner join user_accounts ua on a.id = ua.account_id
            where ua.user_id = ?
            order by a.name"#,
            user_id,
        )
        .fetch_all(pool)
        .await
    }

    pub async fn attach_to_user(
        &self,
        pool: &sqlx::SqlitePool,
        user_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "update user_accounts set is_mine = ? where user_id = ? and account_id = ?",
            self.is_mine,
            user_id,
            self.id,
        )
        .execute(pool)
        .await?;

        Ok(())
    }
}

#[derive(Debug, Clone, Default, FromRow, Serialize, Deserialize)]
pub struct Category {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub user_id: i64,
    pub name: String,
    pub color: Option<String>,
}

impl Category {
    pub async fn create(&mut self, pool: &sqlx::Pool<sqlx::Sqlite>) -> Result<i64, sqlx::Error> {
        self.id = sqlx::query!(
            "insert into categories (user_id, name, color) values (?, ?, ?)",
            self.user_id,
            self.name,
            self.color,
        )
        .execute(pool)
        .await?
        .last_insert_rowid();
        Ok(self.id)
    }

    pub async fn categories_for_user(
        pool: &sqlx::SqlitePool,
        user_id: i64,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            Self,
            r#"select id as 'id!', user_id, name, color
            from categories
            where user_id = ?
            order by name"#,
            user_id,
        )
        .fetch_all(pool)
        .await
    }

    pub async fn ensure(
        pool: &sqlx::Pool<sqlx::Sqlite>,
        user_id: i64,
        category_name: &str,
    ) -> Result<i64, sqlx::Error> {
        Ok(sqlx::query_scalar!(
            "select id as 'id!' from categories where user_id = ? and name = ?",
            user_id,
            category_name,
        )
        .fetch_optional(pool)
        .await?
        .unwrap_or(
            sqlx::query!(
                "insert into categories (user_id, name) values (?, ?)",
                user_id,
                category_name,
            )
            .execute(pool)
            .await?
            .last_insert_rowid(),
        ))
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CsvUploadSession {
    pub file_id: String,
    pub file_path: String,
    pub headers: Vec<String>,
}

#[derive(sqlx::FromRow)]
pub struct DayData {
    pub day: i64,
    pub group_id: Option<i64>,
    pub group_name: Option<String>,
    pub group_color: Option<String>,
    pub transaction_type: String,
    pub total: f64,
}

#[derive(Debug, Serialize)]
pub struct ImportError {
    pub row_number: usize,
    pub row_data: String,
    pub error: String,
}

#[derive(Debug, Serialize)]
pub struct ImportResult {
    pub total_rows: usize,
    pub successful: usize,
    pub failed: usize,
    pub skipped: usize,
    pub errors: Vec<ImportError>,
}

#[derive(Debug, Deserialize)]
pub struct ImportRuleForm {
    pub pattern: String,
    pub category_id: Option<String>,
    pub account_id: Option<String>,
    pub priority: String,
}

#[derive(Debug, Default, sqlx::FromRow, serde::Serialize)]
pub struct ImportRuleWithNames {
    pub id: i64,
    #[allow(dead_code)]
    pub user_id: i64,
    pub pattern: String,
    #[allow(dead_code)]
    pub category_id: Option<i64>,
    pub category_name: Option<String>,
    #[allow(dead_code)]
    pub account_id: Option<i64>,
    pub account_name: Option<String>,
    pub priority: i64,
}

impl ImportRuleWithNames {
    pub async fn create(&mut self, pool: &sqlx::SqlitePool) -> Result<i64, sqlx::Error> {
        let res = sqlx::query!(
            r#"insert into import_rules
            (user_id, pattern, category_id, account_id, priority)
            values (?, ?, ?, ?, ?)"#,
            self.user_id,
            self.pattern,
            self.category_id,
            self.account_id,
            self.priority,
        )
        .execute(pool)
        .await?;
        self.id = res.last_insert_rowid();
        Ok(self.id)
    }

    pub async fn delete(&mut self, pool: &sqlx::SqlitePool) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"delete from import_rules where id = ? and user_id = ?"#,
            self.id,
            self.user_id,
        )
        .execute(pool)
        .await?;
        self.id = 0;
        Ok(())
    }

    pub async fn get(
        pool: &sqlx::SqlitePool,
        user_id: i64,
        rule_id: i64,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            Self,
            r#"select r.id as 'id!', r.user_id as 'user_id!',
              r.pattern, r.category_id, c.name as category_name,
              r.account_id, a.name as account_name, r.priority
            from import_rules r
            left join categories c on r.category_id = c.id
            left join accounts a on r.account_id = a.id
            where r.user_id = ? and r.id = ?
            limit 1"#,
            user_id,
            rule_id,
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn rules_for_user(
        pool: &sqlx::SqlitePool,
        user_id: i64,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            Self,
            r#"select r.id as 'id!', r.user_id as 'user_id!',
              r.pattern, r.category_id, c.name as category_name,
              r.account_id, a.name as account_name, r.priority
            from import_rules r
            left join categories c on r.category_id = c.id
            left join accounts a on r.account_id = a.id
            where r.user_id = ?
            order by c.name asc nulls first, r.priority asc, r.id asc"#,
            user_id
        )
        .fetch_all(pool)
        .await
    }

    pub async fn update(&mut self, pool: &sqlx::SqlitePool) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"update import_rules
            set pattern = ?, category_id = ?, account_id = ?, priority = ?
            where id = ? and user_id = ?"#,
            self.pattern,
            self.category_id,
            self.account_id,
            self.priority,
            self.id,
            self.user_id,
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub fn apply_to_transaction(&self, t: &mut Transaction) -> bool {
        let mut updated = false;
        if t.description
            .to_lowercase()
            .contains(self.pattern.to_lowercase().as_str())
        {
            if self.category_id.is_some() && t.category_id != self.category_id {
                t.category_id = self.category_id;
                updated = true;
            }
            if self.account_id.is_some() && t.account_id != self.account_id {
                t.account_id = self.account_id;
                updated = true;
            }
        }
        updated
    }
}

#[derive(Debug, Clone, Default, FromRow, Serialize, Deserialize)]
pub struct Transaction {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub user_id: i64,
    pub category_id: Option<i64>,
    pub account_id: Option<i64>,
    pub amount: f64,
    #[serde(default)]
    pub original_amount: f64,
    pub description: String,
    pub transaction_date: String,
    #[sqlx(rename = "type")]
    #[serde(rename = "type")]
    pub transaction_type: String,
    pub account: Option<String>,
}

impl Transaction {
    pub async fn chart_data_by_account(
        &self,
        pool: &sqlx::SqlitePool,
    ) -> Result<Vec<DayData>, sqlx::Error> {
        let description = self.description_like_pattern();
        sqlx::query_as!(
            DayData,
            r#"select
              t.account_id as 'group_id!',
              '' as group_color,
              t.type as transaction_type,
              coalesce(a.name, 'no account') as group_name,
              cast(strftime('%d', t.transaction_date) as integer) as 'day!',
              cast(sum(t.amount) as real) as total
            from transactions t
              left join accounts a on t.account_id = a.id
            where t.user_id = ?
              and t.category_id = ?
              and (length(?) = 0 or t.type = ?)
              and (length(?) = 0 or t.description like ?)
              and (length(?) != 7 or strftime('%Y-%m', t.transaction_date) = ?)
              and (length(?) != 4 or strftime('%Y', t.transaction_date) = ?)
            group by 'day!', t.account_id, t.type
            order by 'day!', t.type, total desc
            "#,
            self.user_id,
            self.category_id,
            self.transaction_type,
            self.transaction_date,
            description,
            description,
            self.transaction_date,
            self.transaction_date,
            self.transaction_date,
            self.transaction_type,
        )
        .fetch_all(pool)
        .await
    }

    pub async fn chart_data_by_category(
        &self,
        pool: &sqlx::SqlitePool,
    ) -> Result<Vec<DayData>, sqlx::Error> {
        let description = self.description_like_pattern();
        sqlx::query_as!(
            DayData,
            r#"select
              t.category_id as 'group_id!',
              t.type as transaction_type,
              c.color as group_color,
              coalesce(c.name, 'uncategorized') as group_name,
              cast(strftime('%d', t.transaction_date) as integer) as 'day!',
              cast(sum(t.amount) as real) as total
            from transactions t
            left join categories c on t.category_id = c.id
            where t.user_id = ?
              and (? <= 0 or t.account_id = ?)
              and (length(?) = 0 or t.type = ?)
              and (length(?) = 0 or t.description like ?)
              and (length(?) != 7 or strftime('%Y-%m', t.transaction_date) = ?)
              and (length(?) != 4 or strftime('%Y', t.transaction_date) = ?)
            group by 'day!', t.category_id, t.type
            order by 'day!', t.type, total desc"#,
            self.user_id,
            self.account_id,
            self.account_id,
            self.transaction_type,
            self.transaction_type,
            description,
            description,
            self.transaction_date,
            self.transaction_date,
            self.transaction_date,
            self.transaction_date,
        )
        .fetch_all(pool)
        .await
    }

    pub async fn create(&self, pool: &sqlx::SqlitePool) -> Result<i64, sqlx::Error> {
        let res = sqlx::query!(
            r#"insert into transactions
              (user_id, category_id, account_id, amount, original_amount, description, transaction_date, type, account)
              values (?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
            self.user_id,
            self.category_id,
            self.account_id,
            self.amount,
            self.amount,  // For manual entry, original_amount = amount
            self.description,
            self.transaction_date,
            self.transaction_type,
            self.account,
        )
        .execute(pool)
        .await?;

        Ok(res.last_insert_rowid())
    }

    pub async fn create_or_update(
        &mut self,
        pool: &sqlx::Pool<sqlx::Sqlite>,
    ) -> Result<i64, sqlx::Error> {
        let id = sqlx::query_scalar!(
            "select id as 'i64!' from transactions where user_id = ? and transaction_date = ? and amount = ? and description = ?",
            self.user_id,
            self.transaction_date,
            self.amount,
            self.description,
        )
        .fetch_optional(pool)
        .await?;

        if let Some(id) = id {
            sqlx::query!(
                "update transactions set category_id = ?, account_id = ?, type = ?, account = ? where id = ?",
                self.category_id,
                self.account_id,
                self.transaction_type,
                self.account,
                id,
            )
            .execute(pool)
            .await?;

            self.id = id;
            return Ok(id);
        }

        let id = self.create(pool).await?;
        self.id = id;
        Ok(id)
    }

    fn description_like_pattern(&self) -> String {
        if self.description.trim().is_empty() || self.description.contains("%") {
            "".to_string()
        } else {
            format!("%{}%", self.description)
        }
    }

    pub async fn transactions_for_user(
        pool: &sqlx::SqlitePool,
        user_id: i64,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            Self,
            r#"select
              id as 'id!',
              user_id as 'user_id!',
              account_id,
              account,
              amount,
              category_id,
              description,
              coalesce(original_amount, 0)  as original_amount,
              transaction_date,
              type as transaction_type
            from transactions where user_id = ?"#,
            user_id
        )
        .fetch_all(pool)
        .await
    }

    pub async fn transactions_with_category(
        &self,
        pool: &sqlx::SqlitePool,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<TransactionWithCategory>, sqlx::Error> {
        let description = self.description_like_pattern();
        sqlx::query_as!(
            TransactionWithCategory,
            r#"select
              t.id as 'id!',
              t.amount,
              t.description,
              t.transaction_date,
              t.type as transaction_type,
              t.account,
              t.account_id,
              a.name as account_name,
              c.name as category_name,
              c.color as category_color
            from transactions t
            left join categories c on t.category_id = c.id
            left join accounts a on t.account_id = a.id
            where t.user_id = ?
              and (length(?) = 0 or strftime('%Y-%m', t.transaction_date) = ?)
              and (? <= 0 or t.account_id = ?)
              and (? <= 0 or t.category_id = ?)
              and (? > -1 or t.category_id is null)
              and (length(?) = 0 or t.description like ?)
              and (length(?) = 0 or t.type = ?)
            order by t.transaction_date desc
            limit ? offset ?"#,
            self.user_id,
            self.transaction_date,
            self.transaction_date,
            self.account_id,
            self.account_id,
            self.category_id,
            self.category_id,
            self.category_id,
            description,
            description,
            self.transaction_type,
            self.transaction_type,
            limit,
            offset,
        )
        .fetch_all(pool)
        .await
    }
}

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

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub email: String,
    pub name: String,
    pub oauth_provider: String,
    pub oauth_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CategoryStack {
    pub category_id: Option<i64>,
    pub category_name: String,
    pub category_color: String,
    pub amount: f64,
    pub y_pos: i32,
    pub height: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct DayStack {
    pub day: i64,
    pub income_stacks: Vec<CategoryStack>,
    pub expense_stacks: Vec<CategoryStack>,
    pub total_income: f64,
    pub total_expenses: f64,
}

#[derive(Debug, Serialize)]
pub struct PieSlice {
    pub name: String,
    pub color: String,
    pub amount: f64,
    pub percentage: i32,
    pub start_x: i32,
    pub start_y: i32,
    pub end_x: i32,
    pub end_y: i32,
    pub large_arc: i32,
}

#[derive(Debug, Default, Serialize)]
pub struct ChartData {
    pub days: Vec<DayStack>,
    pub max_income: f64,
    pub max_expenses: f64,
    pub total_income: f64,
    pub total_expenses: f64,
    pub all_categories: Vec<(String, String)>, // (name, color) for legend
    pub income_pie_slices: Vec<PieSlice>,
    pub expense_pie_slices: Vec<PieSlice>,
}

pub fn palette() -> Vec<&'static str> {
    vec![
        "oklch(65% 0.20 250)", // Blue
        "oklch(70% 0.19 145)", // Green
        "oklch(75% 0.20 50)",  // Orange
        "oklch(68% 0.20 320)", // Purple
        "oklch(72% 0.18 180)", // Cyan
        "oklch(70% 0.20 25)",  // Red-Orange
        "oklch(65% 0.15 280)", // Indigo
        "oklch(73% 0.17 85)",  // Yellow-Green
        "oklch(68% 0.18 350)", // Magenta
        "oklch(70% 0.16 200)", // Sky Blue
        "oklch(60% 0.10 270)", // Other (muted purple-gray)
    ]
}
