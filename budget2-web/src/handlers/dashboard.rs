use crate::helpers::*;

#[derive(Template)]
#[template(path = "dashboard/index.html")]
pub struct HomeTemplate {
    ctx: RequestContext,
}

pub async fn get(
    State(state): State<AppState>,
    ctx: RequestContext,
    session: tower_sessions::Session,
) -> HttpResult {
    let Ok(_user) = get_current_user(&state.pool, &session).await else {
        return Ok(axum::response::Redirect::to("/auth/login").into_response());
    };

    let page = HomeTemplate { ctx };
    Ok(Html(page.render()?).into_response())
}
