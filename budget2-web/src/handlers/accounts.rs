use crate::helpers::*;

#[derive(Template)]
#[template(path = "accounts/form.html")]
pub struct AccountsFormTemplate {
    #[allow(dead_code)]
    ctx: RequestContext,
    #[allow(dead_code)]
    user: model::User,
    form: model::Account,
    form_open: bool,
    is_editing: bool,
}

#[derive(Template)]
#[template(path = "accounts/index.html")]
pub struct AccountsListTemplate {
    ctx: RequestContext,
    user: model::User,
    accounts: Vec<model::Account>,
    form: model::Account,
    form_open: bool,
    is_editing: bool,
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

    let form = if id > 0 {
        model::Account::load(&state.pool, id, membership.household_id).await?
    } else {
        Some(model::Account::default())
    };

    let page = AccountsFormTemplate {
        ctx,
        user,
        form: form.unwrap_or_default(),
        form_open: true,
        is_editing: id > 0,
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

    let accounts = model::Account::all(&state.pool, membership.household_id).await?;
    let page = AccountsListTemplate {
        ctx,
        user,
        form: model::Account::default(),
        form_open: accounts.is_empty(),
        is_editing: false,
        accounts,
    };

    Ok(Html(page.render()?).into_response())
}

pub async fn save(
    ctx: RequestContext,
    session: tower_sessions::Session,
    State(state): State<AppState>,
    Form(mut form): Form<model::Account>,
) -> HttpResult {
    let Ok((user, membership)) = get_current_membership(&state.pool, &session).await else {
        return Ok(axum::response::Redirect::to("/auth/login").into_response());
    };

    form.user_id = user.id;
    form.save(&state.pool, membership.household_id).await?;

    let accounts = model::Account::all(&state.pool, membership.household_id).await?;
    let page = AccountsListTemplate {
        ctx,
        user,
        form: model::Account::default(),
        accounts,
        form_open: true,
        is_editing: false,
    };

    Ok(Html(page.render()?).into_response())
}
