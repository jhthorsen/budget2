use super::{Deserialize, Pool, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Account {
    #[serde(default)]
    pub id: i64,
    pub user_id: i64,
    pub name: String,
    pub friendly: String,
    pub description: String,
    #[serde(default)]
    pub owner_name: String,
    #[serde(default)]
    pub owner_email: String,
}

impl Account {
    pub async fn all(pool: &Pool, household_id: i64) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            Self,
            r#"select a.id as 'id!', a.user_id as 'user_id!', a.friendly, a.name, a.description,
            u.name as 'owner_name!', u.email as 'owner_email!'
            from accounts a join users u on u.id = a.user_id
            where a.household_id = ?
            order by a.name"#,
            household_id,
        )
        .fetch_all(pool)
        .await
    }

    pub async fn delete(&self, pool: &Pool, household_id: i64) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "delete from accounts where id = ? and household_id = ?",
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
            r#"select a.id as 'id!', a.user_id as 'user_id!', a.friendly, a.name, a.description,
            u.name as 'owner_name!', u.email as 'owner_email!'
            from accounts a join users u on u.id = a.user_id
            where a.id = ? and a.household_id = ?"#,
            id,
            household_id,
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn save(self, pool: &Pool, household_id: i64) -> Result<Self, sqlx::Error> {
        self.validate()?;

        let name = self.name.trim();
        let mut friendly = self.friendly.trim();
        if friendly.is_empty() {
            friendly = name;
        }

        let id = if self.in_storage() {
            sqlx::query!(
                "update accounts set name = ?, friendly = ?, description = ? where id = ? and household_id = ?",
                name,
                friendly,
                self.description,
                self.id,
                household_id,
            )
            .execute(pool)
            .await?;
            self.id
        } else {
            sqlx::query!(
                "insert into accounts (user_id, household_id, name, friendly, description) values (?, ?, ?, ?, ?)",
                self.user_id,
                household_id,
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
