use crate::helpers::*;
use serde::Deserialize;

#[derive(Deserialize)]
pub(super) struct CategoryForm {
    csrf_token: String,
    id: i64,
    name: String,
    description: String,
}

#[derive(Template)]
#[template(path = "categories/form.html")]
pub struct CategoriesFormTemplate {
    #[allow(dead_code)]
    ctx: RequestContext,
    #[allow(dead_code)]
    user: model::User,
    form: model::Category,
    is_editing: bool,
    csrf_token: String,
}

#[derive(Template)]
#[template(path = "categories/index.html")]
pub struct CategoriesListTemplate {
    ctx: RequestContext,
    user: model::User,
    categories: Vec<model::Category>,
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
        model::Category::load(&state.pool, id, membership.household_id).await?
    } else {
        Some(model::Category::default())
    };
    let csrf_token = csrf_token(&session).await?;

    let page = CategoriesFormTemplate {
        ctx,
        user,
        form: form.unwrap_or_default(),
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

    let categories = model::Category::all(&state.pool, membership.household_id).await?;
    let csrf_token = csrf_token(&session).await?;
    let page = CategoriesListTemplate {
        ctx,
        user,
        categories,
        csrf_token,
    };

    Ok(Html(page.render()?).into_response())
}

pub async fn save(
    ctx: RequestContext,
    session: tower_sessions::Session,
    State(state): State<AppState>,
    Form(form): Form<CategoryForm>,
) -> HttpResult {
    let Ok((user, membership)) = get_current_membership(&state.pool, &session).await else {
        return Ok(axum::response::Redirect::to("/auth/login").into_response());
    };
    if !is_manager(&membership) {
        return Ok(axum::response::Redirect::to("/dashboard").into_response());
    }

    verify_csrf(&session, &form.csrf_token).await?;
    model::Category {
        id: form.id,
        name: form.name,
        description: form.description,
        ..Default::default()
    }
    .save(&state.pool, membership.household_id)
    .await?;

    let categories = model::Category::all(&state.pool, membership.household_id).await?;
    let csrf_token = csrf_token(&session).await?;
    let page = CategoriesListTemplate {
        ctx,
        user,
        categories,
        csrf_token,
    };

    Ok(Html(page.render()?).into_response())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        extract::FromRequest,
        http::{Request, header},
    };

    #[tokio::test]
    async fn save_form_accepts_the_csrf_token() {
        let request = Request::builder()
            .method("POST")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from("csrf_token=token&id=1&name=Food&description="))
            .unwrap();

        let Form(form) = Form::<CategoryForm>::from_request(request, &())
            .await
            .unwrap();
        assert_eq!(form.csrf_token, "token");
        assert_eq!(form.name, "Food");
    }
}
