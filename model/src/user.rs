use super::{Deserialize, Pool, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct User {
    #[serde(default)]
    pub id: i64,
    pub email: String,
    pub name: String,
    pub oauth_provider: String,
    pub oauth_id: String,
}

impl User {
    pub async fn by_oauth_identity(
        pool: &Pool,
        oauth_provider: &str,
        oauth_id: &str,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            User,
            "select id as 'id!', email, name, oauth_provider, oauth_id from users where oauth_provider = ? and oauth_id = ?",
            oauth_provider,
            oauth_id
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn by_email(pool: &Pool, email: &str) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            User,
            "select id as 'id!', email, name, oauth_provider, oauth_id from users where email = ?",
            email
        )
        .fetch_optional(pool)
        .await
    }

    pub fn in_storage(&self) -> bool {
        self.id > 0
    }

    pub async fn load(pool: &Pool, id: i64) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            User,
            "select id as 'id!', email, name, oauth_provider, oauth_id from users where id = ?",
            id
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn save(self, pool: &Pool) -> Result<Self, sqlx::Error> {
        self.validate()?;

        let id = if self.in_storage() {
            sqlx::query!(
                "update users set email = ?, name = ?, oauth_provider = ?, oauth_id = ? where id = ?",
                self.email,
                self.name,
                self.oauth_provider,
                self.oauth_id,
                self.id,
            )
            .execute(pool)
            .await?;
            self.id
        } else {
            sqlx::query!(
                "insert into users (email, name, oauth_provider, oauth_id) values (?, ?, ?, ?)",
                self.email,
                self.name,
                self.oauth_provider,
                self.oauth_id,
            )
            .execute(pool)
            .await?
            .last_insert_rowid()
        };

        Ok(Self { id, ..self })
    }

    pub fn short_name(&self) -> &str {
        let Some(short) = self.name.split_once(" ").or(self.email.split_once("@")) else {
            return "";
        };
        short.0
    }

    fn validate(&self) -> Result<(), sqlx::Error> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn finds_users_by_provider_subject() {
        let pool = crate::build_pool("sqlite::memory:", true).await.unwrap();
        let user = User {
            email: "user@example.com".into(),
            name: "User".into(),
            oauth_provider: "https://issuer.example.com".into(),
            oauth_id: "subject-1".into(),
            ..Default::default()
        }
        .save(&pool)
        .await
        .unwrap();

        assert_eq!(
            User::by_oauth_identity(&pool, "https://issuer.example.com", "subject-1")
                .await
                .unwrap()
                .unwrap()
                .id,
            user.id
        );
        assert!(
            User::by_oauth_identity(&pool, "https://other.example.com", "subject-1")
                .await
                .unwrap()
                .is_none()
        );
    }
}
