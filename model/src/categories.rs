use super::{Deserialize, Pool, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Category {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub household_id: i64,
    pub name: String,
    pub description: String,
}

impl Category {
    pub async fn all(pool: &Pool, household_id: i64) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            Self,
            r#"select id as 'id!', household_id as 'household_id!', name, description
            from categories
            where household_id = ?
            order by name"#,
            household_id,
        )
        .fetch_all(pool)
        .await
    }

    pub async fn delete(&self, pool: &Pool) -> Result<(), sqlx::Error> {
        sqlx::query!("delete from categories where id = ?", self.id)
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
            r#"select id as 'id!', household_id as 'household_id!', name, description
            from categories
            where id = ? and household_id = ?"#,
            id,
            household_id,
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn save(self, pool: &Pool, household_id: i64) -> Result<Self, sqlx::Error> {
        self.validate()?;

        let name = self.name.trim();
        let id = if self.in_storage() {
            sqlx::query!(
                "update categories set name = ?, description = ? where id = ? and household_id = ?",
                name,
                self.description,
                self.id,
                household_id,
            )
            .execute(pool)
            .await?;
            self.id
        } else {
            sqlx::query!(
                "insert into categories (household_id, name, description) values (?, ?, ?)",
                household_id,
                name,
                self.description,
            )
            .execute(pool)
            .await?
            .last_insert_rowid()
        };

        Ok(Self { id, ..self })
    }

    pub fn validate(&self) -> Result<(), sqlx::Error> {
        let len = self.name.trim().len();
        if !(2..=64).contains(&len) {
            return super::invalid("Name must be between 3 and 64 characters long.");
        }

        Ok(())
    }
}
