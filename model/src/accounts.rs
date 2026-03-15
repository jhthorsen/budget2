use super::{Deserialize, Pool, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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

    pub async fn delete(&self, pool: &Pool) -> Result<(), sqlx::Error> {
        sqlx::query!("delete from accounts where id = ?", self.id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub fn in_storage(&self) -> bool {
        self.id > 0
    }

    pub async fn load(pool: &Pool, id: i64) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            Self,
            r#"select id as 'id!', user_id as 'user_id!', friendly, name, description
            from accounts
            where id = ?"#,
            id,
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn save(self, pool: &Pool) -> Result<Self, sqlx::Error> {
        self.validate()?;

        let name = self.name.trim();
        let mut friendly = self.friendly.trim();
        if friendly.is_empty() {
            friendly = name;
        }

        let id = if self.in_storage() {
            sqlx::query!(
                "update accounts set user_id = ?, name = ?, friendly = ?, description = ? where id = ?",
                self.user_id,
                name,
                friendly,
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
                name,
                friendly,
                self.description,
            )
            .execute(pool)
            .await?
            .last_insert_rowid()
        };

        Ok(Self { id, ..self })
    }

    pub fn validate(&self) -> Result<(), sqlx::Error> {
        if self.user_id <= 0 {
            return super::invalid("An account must be connected to a user.");
        }

        let len = self.name.trim().len();
        if (3..=64).contains(&len) {
            return super::invalid("Name must be between 3 and 64 characters long.");
        }

        Ok(())
    }
}
