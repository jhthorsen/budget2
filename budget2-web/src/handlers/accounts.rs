use crate::helpers::*;

#[derive(Template)]
#[template(path = "accounts/form.html")]
pub struct AccountsFormTemplate {
    #[allow(dead_code)]
    ctx: RequestContext,
    form: model::Account,
    form_open: bool,
    is_editing: bool,
}

#[derive(Template)]
#[template(path = "accounts/index.html")]
pub struct AccountsListTemplate {
    ctx: RequestContext,
    accounts: Vec<model::Account>,
    form: model::Account,
    form_open: bool,
    is_editing: bool,
}

pub async fn edit(
    ctx: RequestContext,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> HttpResult {
    let form = if id > 0 {
        model::Account::load(&state.pool, id).await?
    } else {
        Some(model::Account::default())
    };

    let page = AccountsFormTemplate {
        ctx,
        form: form.unwrap_or_default(),
        form_open: true,
        is_editing: id > 0,
    };

    Ok(Html(page.render()?).into_response())
}

pub async fn list(ctx: RequestContext, State(state): State<AppState>) -> HttpResult {
    let accounts = model::Account::all(&state.pool).await?;
    let page = AccountsListTemplate {
        ctx,
        form: model::Account::default(),
        form_open: accounts.is_empty(),
        is_editing: false,
        accounts,
    };

    Ok(Html(page.render()?).into_response())
}

pub async fn save(
    ctx: RequestContext,
    State(state): State<AppState>,
    Form(form): Form<model::Account>,
) -> HttpResult {
    form.save(&state.pool).await?;

    let accounts = model::Account::all(&state.pool).await?;
    let page = AccountsListTemplate {
        ctx,
        form: model::Account::default(),
        accounts,
        form_open: true,
        is_editing: false,
    };

    Ok(Html(page.render()?).into_response())
}
