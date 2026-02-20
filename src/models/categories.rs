use super::{Pool, FromRow, Serialize, Deserialize};

#[derive(Debug, Clone, Default, FromRow, Serialize, Deserialize)]
pub struct Category {
    #[serde(default)]
    pub id: i64,
    pub name: String,
    pub description: String,
}

impl Category {
    pub async fn all(pool: &Pool) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            Self,
            r#"select id as 'id!', name, description
            from categories
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
        sqlx::query!("delete from categories where id = ?", self.id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn load(pool: &Pool, lookup: &Self) -> Result<Option<Self>, sqlx::Error> {
        let found = sqlx::query_as!(
            Self,
            "select id as 'id!', name, description from categories where (? > 0 and id = ?) or name = ?",
            lookup.id,
            lookup.id,
            lookup.name,
        )
        .fetch_optional(pool)
        .await?;

        if let Some(mut found) = found {
            crate::set_if_empty!(found, name, &lookup.name);
            crate::set_if_empty!(found, description, &lookup.description);
            return Ok(Some(found));
        }

        Ok(None)
    }

    pub async fn save(self, pool: &Pool) -> Result<Self, sqlx::Error> {
        let id = if self.id > 0 {
            sqlx::query!(
                "update categories set name = ?, description = ? where id = ?",
                self.name,
                self.description,
                self.id,
            )
            .execute(pool)
            .await?;
            self.id
        } else {
            sqlx::query!(
                "insert into categories (name, description) values (?, ?)",
                self.name,
                self.description,
            )
            .execute(pool)
            .await?
            .last_insert_rowid()
        };

        Ok(Self { id, ..self })
    }
}
