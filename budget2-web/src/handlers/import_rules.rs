use crate::helpers::*;

#[derive(Template)]
#[template(path = "import_rules/form.html")]
pub struct ImportRulesFormTemplate {
    #[allow(dead_code)]
    ctx: RequestContext,
    form: model::ImportRule,
    form_open: bool,
    is_editing: bool,
}

#[derive(Template)]
#[template(path = "import_rules/index.html")]
pub struct ImportRulesListTemplate {
    ctx: RequestContext,
    import_rules: Vec<model::ImportRule>,
    form: model::ImportRule,
    form_open: bool,
    is_editing: bool,
}

pub async fn edit(
    ctx: RequestContext,
    session: tower_sessions::Session,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> HttpResult {
    let Ok(_user) = get_current_user(&state.pool, &session).await else {
        return Ok(axum::response::Redirect::to("/auth/login").into_response());
    };

    let form = if id > 0 {
        model::ImportRule::load(&state.pool, id).await?
    } else {
        Some(model::ImportRule::default())
    };

    let page = ImportRulesFormTemplate {
        ctx,
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
    let Ok(_user) = get_current_user(&state.pool, &session).await else {
        return Ok(axum::response::Redirect::to("/auth/login").into_response());
    };

    let import_rules = model::ImportRule::all(&state.pool).await?;
    let page = ImportRulesListTemplate {
        ctx,
        form: model::ImportRule::default(),
        form_open: import_rules.is_empty(),
        is_editing: false,
        import_rules,
    };

    Ok(Html(page.render()?).into_response())
}

pub async fn save(
    ctx: RequestContext,
    session: tower_sessions::Session,
    State(state): State<AppState>,
    Form(form): Form<model::ImportRule>,
) -> HttpResult {
    let Ok(_user) = get_current_user(&state.pool, &session).await else {
        return Ok(axum::response::Redirect::to("/auth/login").into_response());
    };

    form.save(&state.pool).await?;

    let import_rules = model::ImportRule::all(&state.pool).await?;
    let page = ImportRulesListTemplate {
        ctx,
        form: model::ImportRule::default(),
        import_rules,
        form_open: true,
        is_editing: false,
    };

    Ok(Html(page.render()?).into_response())
}
