use super::{Deserialize, FromRow, Pool, Serialize};

#[derive(Debug, Clone, Default, FromRow, Deserialize, Serialize)]
pub struct Transaction {
    #[serde(default)]
    pub id: i64,
    pub user_id: i64,
    pub account_id: i64,
    pub account_name: String,
    pub category_id: Option<i64>,
    pub category_name: Option<String>,
    #[sqlx(rename = "type")]
    #[serde(rename = "type")]
    pub transaction_type: String,
    pub amount: f64,
    pub original_amount: f64,
    pub original_currency: String,
    pub description: String,
    pub source: String,
    pub processed_at: String,
}

impl Transaction {
    pub async fn delete(&self, pool: &Pool) -> Result<(), sqlx::Error> {
        sqlx::query!("delete from transactions where id = ?", self.id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn load(pool: &Pool, lookup: &Self) -> Result<Option<Self>, sqlx::Error> {
        let found = sqlx::query_as!(
            Self,
            r#"select
              t.id as 'id!',
              t.user_id,
              t.account_id,
              coalesce(a.name, '') as account_name,
              t.category_id,
              c.name as category_name,
              t.type as transaction_type,
              t.amount,
              t.original_amount,
              t.original_currency,
              t.description,
              t.source,
              t.processed_at
            from transactions t
            left join categories c on t.category_id = c.id
            left join accounts a on t.account_id = a.id
            where t.id = ?"#,
            lookup.id,
        )
        .fetch_optional(pool)
        .await?;

        if let Some(mut found) = found {
            crate::set_if_non_zero!(found, user_id, lookup.user_id);
            crate::set_if_non_zero!(found, account_id, lookup.account_id);
            crate::set_if_non_zero!(found, amount, lookup.amount);
            crate::set_if_non_zero!(found, original_amount, lookup.original_amount);
            crate::set_if_none!(found, category_id, lookup.category_id);
            crate::set_if_empty!(found, transaction_type, &lookup.transaction_type);
            crate::set_if_empty!(found, original_currency, &lookup.original_currency);
            crate::set_if_empty!(found, description, &lookup.description);
            crate::set_if_empty!(found, source, &lookup.source);
            crate::set_if_empty!(found, processed_at, &lookup.processed_at);
            return Ok(Some(found));
        }

        Ok(None)
    }

    pub async fn save(self, pool: &Pool) -> Result<Self, sqlx::Error> {
        let id = if self.id > 0 {
            sqlx::query!(
                r#"update transactions set
                    user_id = ?, account_id = ?, category_id = ?,
                    type = ?, amount = ?, original_amount = ?, original_currency = ?,
                    description = ?, source = ?, processed_at = ?
                    where id = ?"#,
                self.user_id,
                self.account_id,
                self.category_id,
                self.transaction_type,
                self.amount,
                self.original_currency,
                self.original_currency,
                self.description,
                self.source,
                self.processed_at,
                self.id,
            )
            .execute(pool)
            .await?;
            self.id
        } else {
            sqlx::query!(
                "insert into transactions (
                    user_id, account_id, category_id,
                    type, amount, original_amount, original_currency,
                    description, source, processed_at)
                values (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                self.user_id,
                self.account_id,
                self.category_id,
                self.transaction_type,
                self.amount,
                self.original_currency,
                self.original_currency,
                self.description,
                self.source,
                self.processed_at,
            )
            .execute(pool)
            .await?
            .last_insert_rowid()
        };

        Ok(Self { id, ..self })
    }

    pub async fn search(
        pool: &Pool,
        query: &TransactionQuery,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Self>, sqlx::Error> {
        let description = query.description_like_pattern();
        sqlx::query_as!(
            Self,
            r#"select
              t.id as 'id!',
              t.user_id,
              t.account_id,
              coalesce(a.name, '') as account_name,
              t.category_id,
              c.name as category_name,
              t.type as transaction_type,
              t.amount,
              t.original_amount,
              t.original_currency,
              t.description,
              t.source,
              t.processed_at
            from transactions t
            left join categories c on t.category_id = c.id
            left join accounts a on t.account_id = a.id
            where (length(?) = 0 or t.type = ?)
              and (length(?) != 7 or strftime('%Y-%m', t.processed_at) = ?)
              and (length(?) != 4 or strftime('%Y', t.processed_at) = ?)
              and (? <= 0 or t.account_id = ?)
              and (? <= 0 or t.category_id = ?)
              and (? > -1 or t.category_id is null)
              and (length(?) = 0 or t.description like ?)
            order by t.processed_at desc
            limit ?
            offset ?"#,
            query.transaction_type,
            query.transaction_type,
            query.processed_at,
            query.processed_at,
            query.processed_at,
            query.processed_at,
            query.account_id,
            query.account_id,
            query.category_id,
            query.category_id,
            query.category_id,
            description,
            description,
            limit,
            offset,
        )
        .fetch_all(pool)
        .await
    }
}

#[derive(Debug, Default, Deserialize, Clone, Serialize)]
pub struct TransactionQuery {
    #[serde(default)]
    pub filtered: bool,
    #[serde(default)]
    pub account_id: i64,
    #[serde(default)]
    pub category_id: i64,
    #[serde(default)]
    pub processed_at: String,
    #[serde(default)]
    pub page: i64,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub transaction_type: String,
}

impl TransactionQuery {
    pub fn description_like_pattern(&self) -> String {
        if self.description.trim().is_empty() || self.description.contains("%") {
            "".to_string()
        } else {
            format!("%{}%", self.description)
        }
    }
}
