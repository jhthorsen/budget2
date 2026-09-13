use crate::helpers::*;

#[derive(Template)]
#[template(path = "import_rules/form.html")]
pub struct ImportRulesFormTemplate {
    #[allow(dead_code)]
    ctx: RequestContext,
    #[allow(dead_code)]
    user: model::User,
    form: model::ImportRule,
    categories: Vec<model::Category>,
    is_editing: bool,
    csrf_token: String,
}

#[derive(Template)]
#[template(path = "import_rules/index.html")]
pub struct ImportRulesListTemplate {
    ctx: RequestContext,
    user: model::User,
    import_rules: Vec<model::ImportRule>,
    csrf_token: String,
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

    let form = if id > 0 {
        model::ImportRule::load(&state.pool, id, membership.household_id).await?
    } else {
        Some(model::ImportRule::default())
    };
    let categories = model::Category::all(&state.pool, membership.household_id).await?;
    let csrf_token = csrf_token(&session).await?;

    let page = ImportRulesFormTemplate {
        ctx,
        user,
        form: form.unwrap_or_default(),
        categories,
        is_editing: id > 0,
        csrf_token,
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

    let import_rules = model::ImportRule::all(&state.pool, membership.household_id).await?;
    let csrf_token = csrf_token(&session).await?;
    let page = ImportRulesListTemplate {
        ctx,
        user,
        import_rules,
        csrf_token,
    };

    Ok(Html(page.render()?).into_response())
}

pub async fn save(
    ctx: RequestContext,
    session: tower_sessions::Session,
    State(state): State<AppState>,
    Form(form): Form<CsrfForm<model::ImportRule>>,
) -> HttpResult {
    let Ok((user, membership)) = get_current_membership(&state.pool, &session).await else {
        return Ok(axum::response::Redirect::to("/auth/login").into_response());
    };
    if !is_manager(&membership) {
        return Ok(axum::response::Redirect::to("/dashboard").into_response());
    }

    verify_csrf(&session, &form.csrf_token).await?;
    let mut rule = form.value;
    rule.household_id = membership.household_id;
    rule.save(&state.pool, membership.household_id).await?;

    let import_rules = model::ImportRule::all(&state.pool, membership.household_id).await?;
    let csrf_token = csrf_token(&session).await?;
    let page = ImportRulesListTemplate {
        ctx,
        user,
        import_rules,
        csrf_token,
    };

    Ok(Html(page.render()?).into_response())
}

pub async fn delete(
    ctx: RequestContext,
    session: tower_sessions::Session,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(form): Form<CsrfForm<()>>,
) -> HttpResult {
    let Ok((user, membership)) = get_current_membership(&state.pool, &session).await else {
        return Ok(axum::response::Redirect::to("/auth/login").into_response());
    };
    verify_csrf(&session, &form.csrf_token).await?;
    if !is_manager(&membership) {
        return Ok(axum::response::Redirect::to("/dashboard").into_response());
    }

    let Some(rule) = model::ImportRule::load(&state.pool, id, membership.household_id).await?
    else {
        return Ok(axum::response::Redirect::to("/import_rules").into_response());
    };
    rule.delete(&state.pool, membership.household_id).await?;

    let import_rules = model::ImportRule::all(&state.pool, membership.household_id).await?;
    let csrf_token = csrf_token(&session).await?;
    let page = ImportRulesListTemplate {
        ctx,
        user,
        import_rules,
        csrf_token,
    };

    Ok(Html(page.render()?).into_response())
}
