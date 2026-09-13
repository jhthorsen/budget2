use crate::helpers::*;
use serde::Deserialize;

#[derive(Template)]
#[template(path = "household.html")]
struct HouseholdTemplate {
    ctx: RequestContext,
    user: model::User,
    members: Vec<model::HouseholdMember>,
    is_manager: bool,
}

#[derive(Debug, Deserialize)]
pub struct RoleForm {
    user_id: i64,
    role: String,
}

pub async fn get(
    ctx: RequestContext,
    session: tower_sessions::Session,
    State(state): State<AppState>,
) -> HttpResult {
    let Ok((user, membership)) = get_current_membership(&state.pool, &session).await else {
        return Ok(axum::response::Redirect::to("/auth/login").into_response());
    };
    let members = model::HouseholdMembership::members(&state.pool, membership.household_id).await?;
    Ok(Html(
        HouseholdTemplate {
            ctx,
            user,
            members,
            is_manager: matches!(membership.role, model::Role::Manager),
        }
        .render()?,
    )
    .into_response())
}

pub async fn set_role(
    session: tower_sessions::Session,
    State(state): State<AppState>,
    Form(form): Form<RoleForm>,
) -> HttpResult {
    let Ok((user, membership)) = get_current_membership(&state.pool, &session).await else {
        return Ok(axum::response::Redirect::to("/auth/login").into_response());
    };
    model::HouseholdMembership::set_role(
        &state.pool,
        membership.household_id,
        user.id,
        form.user_id,
        model::Role::parse(&form.role),
    )
    .await?;
    Ok(axum::response::Redirect::to("/household").into_response())
}
