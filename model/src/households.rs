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
        let manager = sqlx::query_scalar::<_, i64>(
            "select count(*) from household_members where household_id = ? and user_id = ? and role = 'manager'",
        )
        .bind(household_id).bind(actor_id).fetch_one(pool).await?;
        if manager == 0 {
            return Err(sqlx::Error::Protocol(
                "Only household managers can change roles".into(),
            ));
        }
        sqlx::query("update household_members set role = ? where household_id = ? and user_id = ?")
            .bind(role.as_str())
            .bind(household_id)
            .bind(user_id)
            .execute(pool)
            .await?;
        Ok(())
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
