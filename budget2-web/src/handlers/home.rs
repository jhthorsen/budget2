use crate::helpers::*;

#[derive(Template)]
#[template(path = "home/index.html")]
pub struct HomeTemplate {
    ctx: RequestContext,
}

pub async fn get(
    State(state): State<AppState>,
    ctx: RequestContext,
    session: tower_sessions::Session,
) -> HttpResult {
    let page = HomeTemplate { ctx };

    if get_current_user(&state.pool, &session).await.is_ok() {
        Ok(axum::response::Redirect::to("/dashboard").into_response())
    } else {
        Ok(Html(page.render()?).into_response())
    }
}
