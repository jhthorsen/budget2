use crate::helpers::*;
use serde::Deserialize;

const TRANSACTION_ROWS: i64 = 60;
const CHART_COLORS: [&str; 12] = [
    "#2f855a", "#3182ce", "#805ad5", "#d69e2e", "#d53f8c", "#dd6b20", "#319795", "#00a3c4",
    "#5a67d8", "#b83280", "#718096", "#c05621",
];

struct ChartSeries {
    name: String,
    color: &'static str,
    points: Vec<model::LinePoint>,
}

fn chart_series(points: Vec<model::LinePoint>) -> Vec<ChartSeries> {
    let mut series = Vec::new();
    for point in points {
        if let Some(index) = series
            .iter()
            .position(|item: &ChartSeries| item.name == point.series)
        {
            series[index].points.push(point);
        } else {
            series.push(ChartSeries {
                name: point.series.clone(),
                color: "",
                points: vec![point],
            });
        }
    }
    series.sort_unstable_by(|left, right| left.name.cmp(&right.name));
    for (index, item) in series.iter_mut().enumerate() {
        item.color = CHART_COLORS[index % CHART_COLORS.len()];
    }
    series
}

/// Query-string state shared by the dashboard summary, chart, and transaction list.
#[derive(Debug, Default, Deserialize)]
pub struct DashboardQuery {
    /// A month (`YYYY-MM`) or day (`YYYY-MM-DD`) prefix for `processed_at`.
    #[serde(default)]
    date: String,
    /// Case-insensitive substring to find in the transaction description.
    #[serde(default)]
    description: String,
    /// Account selector from the account filter; an empty or invalid value means all accounts.
    #[serde(default)]
    account: String,
    /// Category selector from the category filter; `0` represents uncategorized transactions.
    #[serde(default)]
    category: String,
    /// Optional minimum transaction amount from the dashboard filter form.
    #[serde(default)]
    amount: String,
    /// Number of transactions to skip when loading the next page.
    #[serde(default)]
    offset: i64,
}

impl DashboardQuery {
    fn filters(&self) -> model::TransactionFilters<'_> {
        model::TransactionFilters {
            date: (!self.date.trim().is_empty()).then_some(self.date.trim()),
            description: (!self.description.trim().is_empty()).then_some(self.description.trim()),
            account_id: self.account.parse().ok(),
            category_id: self.category.parse().ok(),
            minimum_amount: self.amount.parse().ok(),
        }
    }

    fn account_id(&self) -> i64 {
        self.account.parse().unwrap_or(-1)
    }

    fn category_id(&self) -> i64 {
        self.category.parse().unwrap_or(-1)
    }
}

/// Values rendered into the full dashboard page.
#[derive(Template)]
#[template(path = "dashboard/index.html")]
pub struct DashboardTemplate {
    ctx: RequestContext,
    user: model::User,
    formatted_income: String,
    formatted_expenses: String,
    transaction_count: i64,
    uncategorized_count: i64,
    transaction_filter_accounts: Vec<model::TransactionFilterOption>,
    transaction_filter_categories: Vec<model::TransactionFilterOption>,
    filters: DashboardQuery,
    chart_series: Vec<ChartSeries>,
    csrf_token: String,
}

/// Values rendered into the partial response used by infinite scrolling.
#[derive(Template)]
#[template(path = "dashboard/transaction_rows.html")]
struct TransactionRowsTemplate {
    transactions: Vec<model::Transaction>,
    has_more_transactions: bool,
    next_transaction_offset: i64,
}

pub async fn get(
    State(state): State<AppState>,
    ctx: RequestContext,
    session: tower_sessions::Session,
    Query(query): Query<DashboardQuery>,
) -> HttpResult {
    let Ok((user, membership)) = get_current_membership(&state.pool, &session).await else {
        return Ok(axum::response::Redirect::to("/auth/login").into_response());
    };

    let report = model::Dashboard::report(
        &state.pool,
        membership.household_id,
        user.id,
        membership.role,
        query.filters(),
    )
    .await?;
    let income = report.formatted_income();
    let expenses = report.formatted_expenses();
    let csrf_token = csrf_token(&session).await?;

    let page = DashboardTemplate {
        ctx,
        user,
        filters: query,
        chart_series: chart_series(report.line_points),
        csrf_token,
        formatted_income: income,
        formatted_expenses: expenses,
        transaction_count: report.transaction_count,
        uncategorized_count: report.uncategorized_count,
        transaction_filter_accounts: report.transaction_filter_accounts,
        transaction_filter_categories: report.transaction_filter_categories,
    };

    Ok(Html(page.render()?).into_response())
}

pub async fn transactions(
    State(state): State<AppState>,
    session: tower_sessions::Session,
    Query(query): Query<DashboardQuery>,
) -> HttpResult {
    let Ok((user, membership)) = get_current_membership(&state.pool, &session).await else {
        return Ok(axum::response::Redirect::to("/auth/login").into_response());
    };

    let page = model::Dashboard::transactions(
        &state.pool,
        membership.household_id,
        user.id,
        membership.role,
        query.filters(),
        query.offset.max(0),
        TRANSACTION_ROWS,
    )
    .await?;
    let page = TransactionRowsTemplate {
        transactions: page.transactions,
        has_more_transactions: page.has_more,
        next_transaction_offset: query.offset.max(0) + TRANSACTION_ROWS,
    };
    Ok(Html(page.render()?).into_response())
}
