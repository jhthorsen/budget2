use crate::{AppState, filters, models::*, request_context::RequestContext};
use askama::Template;
use axum::extract::{Query, State};
use axum::response::IntoResponse;
use sqlx::SqlitePool;
use std::collections::HashMap;

const PER_PAGE: i64 = 100;

#[derive(Template)]
#[template(path = "dashboard.html")]
struct DashboardTemplate {
    ctx: RequestContext,
    accounts: Vec<Account>,
    categories: Vec<Category>,
    chart_data: ChartData,
    filters: TransactionQuery,
    more_transactions: bool,
    transactions: Vec<Transaction>,
    user: User,
}

pub async fn dashboard_handler(
    State(state): State<AppState>,
    session: tower_sessions::Session,
    Query(mut filters): Query<TransactionQuery>,
    ctx: RequestContext,
) -> super::HttpResult {
    let user = auth::get_current_user(&state.pool, &session).await?;
    let offset = (filters.page.max(1) - 1) * PER_PAGE;
    let mut transactions = Transaction::search(&state.pool, &filters, offset, PER_PAGE + 1).await?;

    let more_transactions = if transactions.len() as i64 > PER_PAGE {
        transactions.pop();
        true
    } else {
        false
    };

    let accounts = Account::all(&state.pool).await?;
    let categories = Category::all(&state.pool).await?;

    if filters.processed_at.is_empty() {
        filters.processed_at = match transactions.first() {
            Some(t) => t.processed_at.chars().take(4).collect(),
            None => "1900".to_string(),
        };
    }

    let chart_data = fetch_chart_data(&state.pool, &filters).await?;
    let html = if filters.filtered {
        let template = DashboardSearchResultsTemplate {
            ctx,
            chart_data,
            filters,
            more_transactions,
            transactions,
        };

        template.render()?
    } else {
        let template = DashboardTemplate {
            ctx,
            accounts,
            categories,
            chart_data,
            filters,
            more_transactions,
            transactions,
            user,
        };

        template.render()?
    };

    Ok(axum::response::Html(html).into_response())
}

#[derive(Template)]
#[template(path = "dashboard/search_results.html")]
struct DashboardSearchResultsTemplate {
    ctx: RequestContext,
    chart_data: ChartData,
    filters: TransactionQuery,
    more_transactions: bool,
    transactions: Vec<Transaction>,
}

async fn fetch_chart_data(
    pool: &SqlitePool,
    filters: &TransactionQuery,
) -> Result<ChartData, sqlx::Error> {
    let group_by_account = filters.category_id > 0;
    let chart_data = if group_by_account {
        ChartData::by_account(pool, filters).await?
    } else {
        ChartData::by_category(pool, filters).await?
    };

    let mut by_day: HashMap<String, Vec<DayData>> = HashMap::new();
    let mut legends: HashMap<String, String> = HashMap::new();

    if let Some(start) = chart_data.first() {
        let end = chart_data.last().cloned().unwrap_or(start.clone());
        let end = chrono::NaiveDate::parse_from_str(&end.date, "%Y-%m-%d").unwrap();
        let mut start = chrono::NaiveDate::parse_from_str(&start.date, "%Y-%m-%d").unwrap();
        while start <= end {
            by_day
                .entry(start.format("%Y-%m-%d").to_string())
                .or_default();
            start += chrono::Duration::days(1);
        }
    }

    let mut ci = 0;
    for day in chart_data.iter() {
        let color = match day.color.is_empty() {
            false => day.color.clone(),
            true => {
                ci += 1;
                if ci >= filters::PALETTE.len() {
                    ci = 0;
                }
                filters::PALETTE[ci].to_string()
            }
        };

        legends.insert(day.group_name.clone(), color);
        by_day
            .entry(day.date.clone())
            .or_default()
            .push(day.to_owned());
    }

    Ok(ChartData { by_day, legends })
}
