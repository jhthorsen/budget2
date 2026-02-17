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
    accounts: Vec<AccountWithOwnership>,
    categories: Vec<Category>,
    chart_data: ChartData,
    filters: TransactionFilters,
    more_transactions: bool,
    transactions: Vec<TransactionWithCategory>,
    user: User,
}

#[derive(Template)]
#[template(path = "dashboard/search_results.html")]
struct DashboardSearchResultsTemplate {
    ctx: RequestContext,
    chart_data: ChartData,
    filters: TransactionFilters,
    more_transactions: bool,
    transactions: Vec<TransactionWithCategory>,
}

pub async fn dashboard_handler(
    State(state): State<AppState>,
    session: tower_sessions::Session,
    Query(filters): Query<TransactionFilters>,
    ctx: RequestContext,
) -> crate::HttpResult {
    let user = auth::get_current_user(&state.pool, &session).await?;

    let mut transaction = Transaction {
        account_id: Some(filters.account_id),
        category_id: Some(filters.category_id),
        description: filters.search.trim().to_owned(),
        transaction_type: filters.transaction_type.trim().to_owned(),
        transaction_date: filters.month.trim().to_owned(),
        user_id: user.id,
        ..Transaction::default()
    };

    let offset = (filters.page.max(1) - 1) * PER_PAGE;
    let mut transactions = transaction
        .transactions_with_category(&state.pool, offset, PER_PAGE + 1)
        .await
        .map_err(|err| super::db_error(err, "Unable to fetch transactions"))?;

    let more_transactions = if transactions.len() as i64 > PER_PAGE {
        transactions.pop();
        true
    } else {
        false
    };

    let accounts = AccountWithOwnership::accounts_for_user(&state.pool, user.id)
        .await
        .map_err(|err| super::db_error(err, "Unable to fetch accounts"))?;

    let categories = Category::categories_for_user(&state.pool, user.id)
        .await
        .map_err(|err| super::db_error(err, "Unable to fetch categories"))?;

    if transaction.transaction_date.is_empty() {
        transaction.transaction_date = match transactions.first() {
            Some(t) => t.transaction_date.chars().take(4).collect(),
            None => "1900".to_string(),
        };
    }

    let chart_data = fetch_chart_data(&state.pool, &transaction)
        .await
        .map_err(|err| super::db_error(err, "Unable to fetch chart data"))?;

    let rendered = if filters.filtered {
        let template = DashboardSearchResultsTemplate {
            ctx,
            chart_data,
            filters,
            more_transactions,
            transactions,
        };

        template.render()
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

        template.render()
    };

    Ok(rendered
        .map(axum::response::Html)
        .map_err(super::template_error)?
        .into_response())
}

async fn fetch_chart_data(
    pool: &SqlitePool,
    transaction: &Transaction,
) -> Result<ChartData, sqlx::Error> {
    let group_by_account = transaction.category_id.unwrap_or_default() > 0;
    let chart_data = if group_by_account {
        transaction.chart_data_by_account(pool).await?
    } else {
        transaction.chart_data_by_category(pool).await?
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
