use super::{Deserialize, FromRow, Pool, Serialize};

#[derive(Debug, Clone, Default, FromRow, Serialize, Deserialize)]
pub struct Account {
    #[serde(default)]
    pub id: i64,
    pub user_id: i64,
    pub name: String,
    pub friendly: String,
    pub description: String,
}

impl Account {
    pub async fn all(pool: &Pool) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            Self,
            r#"select id as 'id!', user_id as 'user_id!', friendly, name, description
            from accounts
            order by name"#,
        )
        .fetch_all(pool)
        .await
    }

    pub async fn by_name(pool: &Pool, name: &str) -> Result<Self, sqlx::Error> {
        let lookup = Self {
            name: name.to_owned(),
            ..Self::default()
        };
        Ok(Self::load(pool, &lookup).await?.unwrap_or(lookup))
    }

    pub async fn delete(&self, pool: &Pool) -> Result<(), sqlx::Error> {
        sqlx::query!("delete from accounts where id = ?", self.id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn load(pool: &Pool, lookup: &Self) -> Result<Option<Self>, sqlx::Error> {
        let found = sqlx::query_as!(
            Self,
            "select id as 'id!', user_id as 'user_id!', friendly, name, description from accounts where (? > 0 and id = ?) or name = ?",
            lookup.id,
            lookup.id,
            lookup.name,
        )
        .fetch_optional(pool)
        .await?;

        if let Some(mut found) = found {
            crate::set_if_empty!(found, name, &lookup.name);
            crate::set_if_empty!(found, description, &lookup.description);
            crate::set_if_empty!(found, friendly, &lookup.friendly);
            return Ok(Some(found));
        }

        Ok(None)
    }

    pub async fn save(self, pool: &Pool) -> Result<Self, sqlx::Error> {
        let id = if self.id > 0 {
            sqlx::query!(
                "update accounts set user_id = ?, name = ?, friendly = ?, description = ? where id = ?",
                self.user_id,
                self.name,
                self.friendly,
                self.description,
                self.id,
            )
            .execute(pool)
            .await?;
            self.id
        } else {
            sqlx::query!(
                "insert into accounts (user_id, name, friendly, description) values (?, ?, ?, ?)",
                self.user_id,
                self.name,
                self.friendly,
                self.description,
            )
            .execute(pool)
            .await?
            .last_insert_rowid()
        };

        Ok(Self { id, ..self })
    }
}
