use super::{Pool, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Role {
    Manager,
    Assistant,
    Member,
}

impl Role {
    pub fn parse(value: &str) -> Self {
        match value {
            "manager" => Self::Manager,
            "assistant" => Self::Assistant,
            _ => Self::Member,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Manager => "manager",
            Self::Assistant => "assistant",
            Self::Member => "member",
        }
    }
}

impl fmt::Display for Role {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Manager => "Manager",
            Self::Assistant => "Assistant",
            Self::Member => "Member",
        })
    }
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct HouseholdMembership {
    pub household_id: i64,
    pub user_id: i64,
    pub role: Role,
}

impl HouseholdMembership {
    pub async fn for_user(pool: &Pool, user_id: i64) -> Result<Option<Self>, sqlx::Error> {
        Ok(sqlx::query_as::<_, DbMembership>(
            "select household_id, user_id, role from household_members where user_id = ?",
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await?
        .map(Into::into))
    }

    pub async fn ensure_for_user(pool: &Pool, user_id: i64) -> Result<Self, sqlx::Error> {
        if let Some(membership) = Self::for_user(pool, user_id).await? {
            return Ok(membership);
        }

        let household_id = sqlx::query(
            "insert into households (name) select name || '''s Household' from users where id = ?",
        )
        .bind(user_id)
        .execute(pool)
        .await?
        .last_insert_rowid();
        sqlx::query(
            "insert into household_members (household_id, user_id, role) values (?, ?, 'manager')",
        )
        .bind(household_id)
        .bind(user_id)
        .execute(pool)
        .await?;
        Ok(Self {
            household_id,
            user_id,
            role: Role::Manager,
        })
    }

    pub async fn members(
        pool: &Pool,
        household_id: i64,
    ) -> Result<Vec<HouseholdMember>, sqlx::Error> {
        Ok(sqlx::query_as::<_, DbHouseholdMember>(
            "select m.user_id, u.email, u.name, m.role from household_members m join users u on u.id = m.user_id where m.household_id = ? order by u.name, u.email",
        )
        .bind(household_id)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(Into::into)
        .collect())
    }

    pub async fn set_role(
        pool: &Pool,
        household_id: i64,
        actor_id: i64,
        user_id: i64,
        role: Role,
    ) -> Result<(), sqlx::Error> {
        let updated = sqlx::query(
            r#"update household_members set role = ?
            where household_id = ? and user_id = ?
              and exists (
                select 1 from household_members actor
                where actor.household_id = ? and actor.user_id = ? and actor.role = 'manager'
              )
              and (
                role != 'manager' or ? = 'manager' or exists (
                  select 1 from household_members other
                  where other.household_id = ? and other.role = 'manager' and other.user_id != ?
                )
              )"#,
        )
        .bind(role.as_str())
        .bind(household_id)
        .bind(user_id)
        .bind(household_id)
        .bind(actor_id)
        .bind(role.as_str())
        .bind(household_id)
        .bind(user_id)
        .execute(pool)
        .await?;
        if updated.rows_affected() == 0 {
            return Err(sqlx::Error::Protocol(
                "Only household managers can change roles, and a household must retain a manager"
                    .into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn household_retains_a_manager() {
        let pool = crate::build_pool("sqlite::memory:", true).await.unwrap();
        sqlx::query("insert into users (id, email, name, oauth_provider, oauth_id) values (1, 'one@example.com', 'One', 'test', 'one'), (2, 'two@example.com', 'Two', 'test', 'two')")
            .execute(&pool).await.unwrap();
        sqlx::query("insert into households (id, name) values (1, 'Home')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("insert into household_members (household_id, user_id, role) values (1, 1, 'manager'), (1, 2, 'member')")
            .execute(&pool).await.unwrap();

        assert!(
            HouseholdMembership::set_role(&pool, 1, 1, 1, Role::Member)
                .await
                .is_err()
        );
        HouseholdMembership::set_role(&pool, 1, 1, 2, Role::Manager)
            .await
            .unwrap();
        HouseholdMembership::set_role(&pool, 1, 1, 1, Role::Member)
            .await
            .unwrap();
    }
}

#[derive(Debug, sqlx::FromRow)]
struct DbMembership {
    household_id: i64,
    user_id: i64,
    role: String,
}

impl From<DbMembership> for HouseholdMembership {
    fn from(value: DbMembership) -> Self {
        Self {
            household_id: value.household_id,
            user_id: value.user_id,
            role: Role::parse(&value.role),
        }
    }
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct HouseholdMember {
    pub user_id: i64,
    pub email: String,
    pub name: String,
    pub role: Role,
}

#[derive(Debug, sqlx::FromRow)]
struct DbHouseholdMember {
    user_id: i64,
    email: String,
    name: String,
    role: String,
}

impl From<DbHouseholdMember> for HouseholdMember {
    fn from(value: DbHouseholdMember) -> Self {
        Self {
            user_id: value.user_id,
            email: value.email,
            name: value.name,
            role: Role::parse(&value.role),
        }
    }
}
