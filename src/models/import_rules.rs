use super::{Deserialize, FromRow, Pool, Serialize};

#[derive(Debug, Default, FromRow, Deserialize, Serialize)]
pub struct ImportRule {
    #[serde(default)]
    pub id: i64,
    pub priority: i64,
    pub match_description: Option<String>,
    pub match_account: Option<String>,
    pub match_category: Option<String>,
    pub account_id: Option<i64>,
    pub account_name: Option<String>,
    pub category_id: Option<i64>,
    pub category_name: Option<String>,
}

impl ImportRule {
    pub async fn all(pool: &Pool) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            Self,
            r#"select
                r.id as 'id!', r.priority,
                r.match_description, r.match_account, r.match_category,
                r.account_id, a.name as account_name,
                r.category_id, c.name as category_name
            from import_rules r
            left join categories c on r.category_id = c.id
            left join accounts a on r.account_id = a.id
            order by c.name asc nulls first, r.priority asc, r.id asc"#,
        )
        .fetch_all(pool)
        .await
    }

    pub fn apply_to_transaction(&self, t: &mut super::Transaction) -> bool {
        let update = if let Some(m) = self.match_description.as_ref()
            && t.description
                .to_lowercase()
                .contains(m.to_lowercase().as_str())
        {
            true
        } else if let Some(m) = self.match_account.as_ref()
            && "TODO" == m.to_lowercase().as_str()
        {
            true
        } else if let Some(m) = self.match_category.as_ref()
            && "TODO" == m.to_lowercase().as_str()
        {
            true
        } else {
            false
        };

        let mut updated = false;
        if update {
            if let Some(id) = self.account_id {
                t.account_id = id;
                updated = true;
            }
            if let Some(id) = self.category_id {
                t.category_id = Some(id);
                updated = true;
            }
        }

        updated
    }

    pub async fn delete(&self, pool: &Pool) -> Result<(), sqlx::Error> {
        sqlx::query!("delete from import_rules where id = ?", self.id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn load(pool: &Pool, lookup: &Self) -> Result<Option<Self>, sqlx::Error> {
        let found = sqlx::query_as!(
            Self,
            r#"select
                r.id as 'id!', r.priority,
                r.match_description, r.match_account, r.match_category,
                r.account_id, a.name as account_name,
                r.category_id, c.name as category_name
            from import_rules r
            left join categories c on r.category_id = c.id
            left join accounts a on r.account_id = a.id
            where r.id = ?
            order by c.name asc nulls first, r.priority asc, r.id asc"#,
            lookup.id,
        )
        .fetch_optional(pool)
        .await?;

        if let Some(mut found) = found {
            crate::set_if_non_zero!(found, priority, lookup.priority);
            crate::set_if_none!(found, match_description, lookup.match_description);
            crate::set_if_none!(found, match_account, lookup.match_account);
            crate::set_if_none!(found, match_category, lookup.match_category);
            crate::set_if_none!(found, account_id, lookup.account_id);
            crate::set_if_none!(found, account_name, lookup.account_name);
            crate::set_if_none!(found, category_id, lookup.category_id);
            crate::set_if_none!(found, category_name, lookup.category_name);
            return Ok(Some(found));
        }

        Ok(None)
    }

    pub async fn save(self, pool: &Pool) -> Result<Self, sqlx::Error> {
        let id = if self.id > 0 {
            sqlx::query!(
                r#"update import_rules set priority = ?,
                    match_description = ?, match_account = ?, match_category = ?,
                    account_id = ?, category_id = ?
                    where id = ?"#,
                self.priority,
                self.match_description,
                self.match_account,
                self.match_category,
                self.account_id,
                self.category_id,
                self.id,
            )
            .execute(pool)
            .await?;
            self.id
        } else {
            sqlx::query!(
                r#"insert into import_rules (priority,
                    match_description, match_account, match_category,
                    account_id, category_id)
                    values (?, ?, ?, ?, ?, ?)"#,
                self.priority,
                self.match_description,
                self.match_account,
                self.match_category,
                self.account_id,
                self.category_id,
            )
            .execute(pool)
            .await?
            .last_insert_rowid()
        };

        Ok(Self { id, ..self })
    }
}
