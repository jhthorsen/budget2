use super::{Deserialize, Pool, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportRule {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub household_id: i64,
    pub match_account: Option<String>,
    pub match_description: Option<String>,
    pub account_id: Option<i64>,
    pub category_id: Option<i64>,
    pub priority: i64,
    #[serde(default)]
    pub category_name: String,
}

impl Default for ImportRule {
    fn default() -> Self {
        Self {
            id: 0,
            household_id: 0,
            match_account: None,
            match_description: None,
            account_id: None,
            category_id: None,
            priority: 50,
            category_name: String::new(),
        }
    }
}

impl ImportRule {
    pub async fn uncategorized_match_count(
        &self,
        pool: &Pool,
        household_id: i64,
    ) -> Result<i64, sqlx::Error> {
        let Some(description) = self.match_description.as_deref() else {
            return Ok(0);
        };
        sqlx::query_scalar(
            "select count(*) from transactions t join accounts a on a.id = t.account_id where a.household_id = ? and t.category_id is null and instr(lower(t.description), lower(?)) > 0",
        )
        .bind(household_id)
        .bind(description)
        .fetch_one(pool)
        .await
    }

    pub async fn match_uncategorized(
        &self,
        pool: &Pool,
        household_id: i64,
    ) -> Result<u64, sqlx::Error> {
        let Some(category_id) = self.category_id else {
            return Ok(0);
        };
        let Some(description) = self.match_description.as_deref() else {
            return Ok(0);
        };
        sqlx::query(
            "update transactions set category_id = ? where category_id is null and id in (select t.id from transactions t join accounts a on a.id = t.account_id where a.household_id = ? and instr(lower(t.description), lower(?)) > 0)",
        )
        .bind(category_id)
        .bind(household_id)
        .bind(description)
        .execute(pool)
        .await
        .map(|result| result.rows_affected())
    }

    pub async fn all(pool: &Pool, household_id: i64) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            Self,
            r#"select r.id as 'id!', r.household_id as 'household_id!', r.match_account, r.match_description,
            r.account_id, r.category_id, r.priority,
            coalesce(c.name, '') as 'category_name!: String'
            from import_rules r
            left join accounts a on a.id = r.account_id and a.household_id = r.household_id
            left join categories c on c.id = r.category_id and c.household_id = r.household_id
            where r.household_id = ?
            order by r.priority, r.id"#,
            household_id,
        )
        .fetch_all(pool)
        .await
    }

    pub async fn delete(&self, pool: &Pool, household_id: i64) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "delete from import_rules where id = ? and household_id = ?",
            self.id,
            household_id
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub fn in_storage(&self) -> bool {
        self.id > 0
    }

    pub async fn load(
        pool: &Pool,
        id: i64,
        household_id: i64,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            Self,
            r#"select r.id as 'id!', r.household_id as 'household_id!', r.match_account, r.match_description,
            r.account_id, r.category_id, r.priority,
            coalesce(c.name, '') as 'category_name!: String'
            from import_rules r
            left join accounts a on a.id = r.account_id and a.household_id = r.household_id
            left join categories c on c.id = r.category_id and c.household_id = r.household_id
            where r.id = ? and r.household_id = ?"#,
            id,
            household_id,
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn save(self, pool: &Pool, household_id: i64) -> Result<Self, sqlx::Error> {
        self.validate()?;
        if let Some(account_id) = self.account_id {
            let exists = sqlx::query_scalar::<_, i64>(
                "select count(*) from accounts where id = ? and household_id = ?",
            )
            .bind(account_id)
            .bind(household_id)
            .fetch_one(pool)
            .await?;
            if exists == 0 {
                return Err(sqlx::Error::Protocol(
                    "Account is outside the household".into(),
                ));
            }
        }
        if let Some(category_id) = self.category_id {
            let exists = sqlx::query_scalar::<_, i64>(
                "select count(*) from categories where id = ? and household_id = ?",
            )
            .bind(category_id)
            .bind(household_id)
            .fetch_one(pool)
            .await?;
            if exists == 0 {
                return Err(sqlx::Error::Protocol(
                    "Category is outside the household".into(),
                ));
            }
        }
        let id = if self.in_storage() {
            sqlx::query!(
                "update import_rules set match_account = ?, match_description = ?, account_id = ?, category_id = ?, priority = ? where id = ? and household_id = ?",
                self.match_account,
                self.match_description,
                self.account_id,
                self.category_id,
                self.priority,
                self.id,
                household_id,
            )
            .execute(pool)
            .await?;
            self.id
        } else {
            sqlx::query!(
                "insert into import_rules (household_id, match_account, match_description, account_id, category_id, priority) values (?, ?, ?, ?, ?, ?)",
                household_id,
                self.match_account,
                self.match_description,
                self.account_id,
                self.category_id,
                self.priority,
            )
            .execute(pool)
            .await?
            .last_insert_rowid()
        };

        Ok(Self { id, ..self })
    }

    pub fn validate(&self) -> Result<(), sqlx::Error> {
        if !(1..=100).contains(&self.priority) {
            return super::invalid("Priority must be between 1 and 100.");
        }
        if self.match_account.is_none() && self.match_description.is_none() {
            return super::invalid("At least match account or match description must be provided.");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn matches_only_uncategorized_household_transactions() {
        let pool = crate::build_pool("sqlite::memory:", true).await.unwrap();
        for statement in [
            "insert into users (id, email, name, oauth_provider, oauth_id) values (1, 'rules@example.com', 'Rules', 'test', 'rules')",
            "insert into households (id, name) values (1, 'Home'), (2, 'Elsewhere')",
            "insert into accounts (id, user_id, household_id, name) values (1, 1, 1, 'Home account'), (2, 1, 2, 'Other account')",
            "insert into categories (id, household_id, name) values (1, 1, 'Food')",
            "insert into transactions (user_id, account_id, category_id, type, amount, original_amount, description, processed_at) values (1, 1, null, 'expense', 1, 1, 'Coffee shop', '2026-01-01'), (1, 1, 1, 'expense', 1, 1, 'Coffee beans', '2026-01-01'), (1, 2, null, 'expense', 1, 1, 'Coffee shop', '2026-01-01')",
        ] {
            sqlx::query(statement).execute(&pool).await.unwrap();
        }
        let rule = ImportRule {
            category_id: Some(1),
            match_description: Some("coffee".to_string()),
            ..Default::default()
        };

        assert_eq!(rule.uncategorized_match_count(&pool, 1).await.unwrap(), 1);
        assert_eq!(rule.match_uncategorized(&pool, 1).await.unwrap(), 1);
        assert_eq!(rule.uncategorized_match_count(&pool, 1).await.unwrap(), 0);
    }
}
