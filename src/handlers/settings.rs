use crate::{models::*, request_context::RequestContext, AppState};
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
) -> crate::HttpResult {
    let user = auth::get_current_user(&session, &state.pool).await?;

    let categories =
        sqlx::query_as::<_, Category>("SELECT * FROM categories WHERE user_id = ? ORDER BY name")
            .bind(user.id)
            .fetch_all(&state.pool)
            .await
            .map_err(|err| super::db_error(err, "Unable to get list of categories"))?;

    let accounts = sqlx::query_as::<_, AccountWithOwnership>(
        r#"
        SELECT a.id, a.name, a.description, ua.is_mine
        FROM accounts a
        INNER JOIN user_accounts ua ON a.id = ua.account_id
        WHERE ua.user_id = ?
        ORDER BY a.name
        "#,
    )
    .bind(user.id)
    .fetch_all(&state.pool)
    .await
    .map_err(|err| super::db_error(err, "Unable to get list of accounts"))?;

    let template = SettingsTemplate {
        accounts,
        categories,
        ctx,
        user,
    };

    Ok(template
        .render()
        .map(axum::response::Html)
        .map_err(super::template_error)?
        .into_response())
}
