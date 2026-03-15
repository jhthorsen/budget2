use crate::helpers::*;

#[derive(Template)]
#[template(path = "home/index.html")]
pub struct HomeTemplate {
    ctx: RequestContext,
}

pub async fn home(ctx: RequestContext) -> HttpResult {
    let page = HomeTemplate { ctx };
    Ok(Html(page.render()?).into_response())
}
