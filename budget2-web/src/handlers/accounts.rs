use crate::helpers::*;

#[derive(Template)]
#[template(path = "accounts/form.html")]
pub struct AccountsFormTemplate {
    #[allow(dead_code)]
    ctx: RequestContext,
    #[allow(dead_code)]
    user: model::User,
    form: model::Account,
    owners: Vec<model::HouseholdMember>,
}

#[derive(Template)]
#[template(path = "accounts/index.html")]
pub struct AccountsListTemplate {
    ctx: RequestContext,
    user: model::User,
    accounts: Vec<model::Account>,
}

pub async fn edit(
    ctx: RequestContext,
    session: tower_sessions::Session,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> HttpResult {
    let Ok((user, membership)) = get_current_membership(&state.pool, &session).await else {
        return Ok(axum::response::Redirect::to("/auth/login").into_response());
    };
    if !is_manager(&membership) {
        return Ok(axum::response::Redirect::to("/dashboard").into_response());
    }

    if id <= 0 {
        return Ok(axum::response::Redirect::to("/accounts").into_response());
    }
    let Some(form) = model::Account::load(&state.pool, id, membership.household_id).await? else {
        return Ok(axum::response::Redirect::to("/accounts").into_response());
    };
    let owners = model::HouseholdMembership::members(&state.pool, membership.household_id).await?;

    let page = AccountsFormTemplate {
        ctx,
        user,
        form,
        owners,
    };

    Ok(Html(page.render()?).into_response())
}

pub async fn list(
    ctx: RequestContext,
    session: tower_sessions::Session,
    State(state): State<AppState>,
) -> HttpResult {
    let Ok((user, membership)) = get_current_membership(&state.pool, &session).await else {
        return Ok(axum::response::Redirect::to("/auth/login").into_response());
    };
    if !is_manager(&membership) {
        return Ok(axum::response::Redirect::to("/dashboard").into_response());
    }

    let accounts = model::Account::all(&state.pool, membership.household_id).await?;
    let page = AccountsListTemplate {
        ctx,
        user,
        accounts,
    };

    Ok(Html(page.render()?).into_response())
}

pub async fn save(
    ctx: RequestContext,
    session: tower_sessions::Session,
    State(state): State<AppState>,
    Form(form): Form<model::Account>,
) -> HttpResult {
    let Ok((user, membership)) = get_current_membership(&state.pool, &session).await else {
        return Ok(axum::response::Redirect::to("/auth/login").into_response());
    };
    if !is_manager(&membership) {
        return Ok(axum::response::Redirect::to("/dashboard").into_response());
    }

    form.save(&state.pool, membership.household_id).await?;

    let accounts = model::Account::all(&state.pool, membership.household_id).await?;
    let page = AccountsListTemplate {
        ctx,
        user,
        accounts,
    };

    Ok(Html(page.render()?).into_response())
}
