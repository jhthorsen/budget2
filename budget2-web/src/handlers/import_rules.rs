use crate::helpers::*;
use serde::Deserialize;

#[derive(Deserialize)]
pub(super) struct ImportRuleForm {
    csrf_token: String,
    id: i64,
    category_id: String,
    priority: i64,
    match_description: Option<String>,
}

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
    uncategorized_matches: i64,
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
    let uncategorized_matches = match form.as_ref() {
        Some(rule) => {
            rule.uncategorized_match_count(&state.pool, membership.household_id)
                .await?
        }
        None => 0,
    };
    let csrf_token = csrf_token(&session).await?;

    let page = ImportRulesFormTemplate {
        ctx,
        user,
        form: form.unwrap_or_default(),
        categories,
        is_editing: id > 0,
        uncategorized_matches,
        csrf_token,
    };

    Ok(Html(page.render()?).into_response())
}

pub async fn match_uncategorized(
    ctx: RequestContext,
    session: tower_sessions::Session,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(form): Form<ImportRuleForm>,
) -> HttpResult {
    let Ok((user, membership)) = get_current_membership(&state.pool, &session).await else {
        return Ok(axum::response::Redirect::to("/auth/login").into_response());
    };
    if !is_manager(&membership) {
        return Ok(axum::response::Redirect::to("/dashboard").into_response());
    }
    verify_csrf(&session, &form.csrf_token).await?;

    let Some(mut rule) = model::ImportRule::load(&state.pool, id, membership.household_id).await?
    else {
        return Ok(axum::response::Redirect::to("/import_rules").into_response());
    };
    rule.category_id = (!form.category_id.is_empty())
        .then(|| form.category_id.parse())
        .transpose()
        .map_err(|_| "Invalid category")?;
    rule.priority = form.priority;
    rule.match_description = form.match_description;
    rule = rule.save(&state.pool, membership.household_id).await?;
    rule.match_uncategorized(&state.pool, membership.household_id)
        .await?;

    let categories = model::Category::all(&state.pool, membership.household_id).await?;
    let uncategorized_matches = rule
        .uncategorized_match_count(&state.pool, membership.household_id)
        .await?;
    let csrf_token = csrf_token(&session).await?;
    let page = ImportRulesFormTemplate {
        ctx,
        user,
        form: rule,
        categories,
        is_editing: true,
        uncategorized_matches,
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
    Form(form): Form<ImportRuleForm>,
) -> HttpResult {
    let Ok((user, membership)) = get_current_membership(&state.pool, &session).await else {
        return Ok(axum::response::Redirect::to("/auth/login").into_response());
    };
    if !is_manager(&membership) {
        return Ok(axum::response::Redirect::to("/dashboard").into_response());
    }

    verify_csrf(&session, &form.csrf_token).await?;
    let is_editing = form.id > 0;
    let mut rule = model::ImportRule {
        id: form.id,
        category_id: (!form.category_id.is_empty())
            .then(|| form.category_id.parse())
            .transpose()
            .map_err(|_| "Invalid category")?,
        priority: form.priority,
        match_description: form.match_description,
        ..Default::default()
    };
    rule.household_id = membership.household_id;
    rule = rule.save(&state.pool, membership.household_id).await?;

    if is_editing {
        let categories = model::Category::all(&state.pool, membership.household_id).await?;
        let uncategorized_matches = rule
            .uncategorized_match_count(&state.pool, membership.household_id)
            .await?;
        let csrf_token = csrf_token(&session).await?;
        let page = ImportRulesFormTemplate {
            ctx,
            user,
            form: rule,
            categories,
            is_editing,
            uncategorized_matches,
            csrf_token,
        };

        return Ok(Html(page.render()?).into_response());
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

pub async fn delete(
    ctx: RequestContext,
    session: tower_sessions::Session,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(form): Form<CsrfTokenForm>,
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
