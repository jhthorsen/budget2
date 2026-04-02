use crate::helpers::*;

#[derive(Template)]
#[template(path = "categories/form.html")]
pub struct CategoriesFormTemplate {
    #[allow(dead_code)]
    ctx: RequestContext,
    form: model::Category,
    form_open: bool,
    is_editing: bool,
}

#[derive(Template)]
#[template(path = "categories/index.html")]
pub struct CategoriesListTemplate {
    ctx: RequestContext,
    categories: Vec<model::Category>,
    form: model::Category,
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
        model::Category::load(&state.pool, id).await?
    } else {
        Some(model::Category::default())
    };

    let page = CategoriesFormTemplate {
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

    let categories = model::Category::all(&state.pool).await?;
    let page = CategoriesListTemplate {
        ctx,
        form: model::Category::default(),
        form_open: categories.is_empty(),
        is_editing: false,
        categories,
    };

    Ok(Html(page.render()?).into_response())
}

pub async fn save(
    ctx: RequestContext,
    session: tower_sessions::Session,
    State(state): State<AppState>,
    Form(form): Form<model::Category>,
) -> HttpResult {
    let Ok(_user) = get_current_user(&state.pool, &session).await else {
        return Ok(axum::response::Redirect::to("/auth/login").into_response());
    };

    form.save(&state.pool).await?;

    let categories = model::Category::all(&state.pool).await?;
    let page = CategoriesListTemplate {
        ctx,
        form: model::Category::default(),
        categories,
        form_open: true,
        is_editing: false,
    };

    Ok(Html(page.render()?).into_response())
}
