use crate::{AppState, models::*, request_context::RequestContext};
use askama::Template;
use axum::extract::State;
use axum::response::IntoResponse;

#[derive(Template)]
#[template(path = "settings.html")]
struct SettingsTemplate {
    accounts: Vec<AccountWithOwnership>,
    categories: Vec<Category>,
    ctx: RequestContext,
    user: User,
}

pub async fn settings_page(
    State(state): State<AppState>,
    session: tower_sessions::Session,
    ctx: RequestContext,
) -> super::HttpResult {
    let user = auth::get_current_user(&state.pool, &session).await?;
    let template = SettingsTemplate {
        accounts: AccountWithOwnership::accounts_for_user(&state.pool, user.id).await?,
        categories: Category::categories_for_user(&state.pool, user.id).await?,
        ctx,
        user,
    };

    Ok(axum::response::Html(template.render()?).into_response())
}
