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
        }
    }
}

impl ImportRule {
    pub async fn all(pool: &Pool, household_id: i64) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            Self,
            r#"select id as 'id!', household_id as 'household_id!', match_account, match_description, account_id, category_id, priority
            from import_rules
            where household_id = ?
            order by priority, id"#,
            household_id,
        )
        .fetch_all(pool)
        .await
    }

    pub async fn delete(&self, pool: &Pool) -> Result<(), sqlx::Error> {
        sqlx::query!("delete from import_rules where id = ?", self.id)
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
            r#"select id as 'id!', household_id as 'household_id!', match_account, match_description, account_id, category_id, priority
            from import_rules
            where id = ? and household_id = ?"#,
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
        if self.match_account.is_none() && self.match_description.is_none() {
            return super::invalid("At least match account or match description must be provided.");
        }

        Ok(())
    }
}
