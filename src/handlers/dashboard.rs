use crate::{filters, models::*, request_context::RequestContext, AppState};
use askama::Template;
use axum::extract::{Query, State};
use axum::response::IntoResponse;
use sqlx::SqlitePool;

#[derive(Template)]
#[template(path = "dashboard.html")]
struct DashboardTemplate {
    ctx: RequestContext,
    accounts: Vec<AccountWithOwnership>,
    categories: Vec<Category>,
    filtered_summary: BudgetSummary,
    filters: TransactionFilters,
    more_transactions: bool,
    summary: BudgetSummary,
    transactions: Vec<TransactionWithCategory>,
    user: User,
}

#[derive(Template)]
#[template(path = "dashboard/search_results.html")]
struct DashboardSearchResultsTemplate {
    ctx: RequestContext,
    filtered_summary: BudgetSummary,
    filters: TransactionFilters,
    more_transactions: bool,
    summary: BudgetSummary,
    transactions: Vec<TransactionWithCategory>,
}

#[derive(sqlx::FromRow)]
struct FilteredSummaryRow {
    total_income: f64,
    total_expenses: f64,
}

#[derive(sqlx::FromRow)]
struct SummaryRow {
    total_income: f64,
    total_expenses: f64,
}

pub async fn dashboard_handler(
    State(state): State<AppState>,
    session: tower_sessions::Session,
    Query(filters): Query<TransactionFilters>,
    ctx: RequestContext,
) -> crate::HttpResult {
    let user = auth::get_current_user(&session, &state.pool).await?;

    const PER_PAGE: i64 = 100;
    let page = filters.page.max(1);
    let offset = (page - 1) * PER_PAGE;

    // Build dynamic WHERE clause
    let mut where_clauses = vec!["t.user_id = ?".to_string()];
    let mut params: Vec<String> = vec![user.id.to_string()];

    if !filters.search.trim().is_empty() {
        where_clauses.push("t.description LIKE ?".to_string());
        params.push(format!("%{}%", filters.search.trim()));
    }

    if !filters.transaction_type.is_empty() {
        where_clauses.push("t.type = ?".to_string());
        params.push(filters.transaction_type.clone());
    }

    match filters.category_id {
        0 => {}
        -1 => {
            where_clauses.push("t.category_id IS NULL".to_string());
        }
        cat_id => {
            where_clauses.push("t.category_id = ?".to_string());
            params.push(cat_id.to_string());
        }
    };

    if filters.account_id > 0 {
        where_clauses.push("t.account_id = ?".to_string());
        params.push(filters.account_id.to_string());
    }

    if !filters.month.trim().is_empty() {
        where_clauses.push("strftime('%Y-%m', t.transaction_date) = ?".to_string());
        params.push(filters.month.trim().to_string());
    }

    let where_clause = where_clauses.join(" AND ");

    // Get transactions
    let select_query = format!(
        r#"
        SELECT 
            t.id,
            t.amount,
            t.description,
            t.transaction_date,
            t.type as transaction_type,
            t.account,
            t.account_id,
            a.name as account_name,
            c.name as category_name,
            c.color as category_color
        FROM transactions t
        LEFT JOIN categories c ON t.category_id = c.id
        LEFT JOIN accounts a ON t.account_id = a.id
        WHERE {}
        ORDER BY t.transaction_date DESC, t.created_at DESC
        LIMIT ? OFFSET ?
        "#,
        where_clause
    );

    let mut query = sqlx::query_as::<_, TransactionWithCategory>(&select_query);
    for param in &params {
        query = query.bind(param);
    }
    let mut transactions = query
        .bind(PER_PAGE + 1)
        .bind(offset)
        .fetch_all(&state.pool)
        .await
        .map_err(|err| super::db_error(err, "Unable to get list of transactions"))?;

    let more_transactions = if transactions.len() as i64 > PER_PAGE {
        transactions.pop();
        true
    } else {
        false
    };

    let categories =
        sqlx::query_as::<_, Category>("SELECT * FROM categories WHERE user_id = ? ORDER BY name")
            .bind(user.id)
            .fetch_all(&state.pool)
            .await
            .map_err(|err| super::db_error(err, "Unable to get list of categories"))?;

    // Get accounts accessible to this user with ownership status
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

    let summary = calculate_summary(&state.pool, user.id)
        .await
        .map_err(|err| super::db_error(err, "Unable to calculate summary"))?;

    // Calculate filtered summary
    let filtered_summary_query = format!(
        r#"
        SELECT 
            CAST(COALESCE(SUM(CASE WHEN t.type = 'income' THEN t.amount ELSE 0 END), 0.0) AS REAL) as total_income,
            CAST(COALESCE(SUM(CASE WHEN t.type = 'expense' THEN t.amount ELSE 0 END), 0.0) AS REAL) as total_expenses
        FROM transactions t
        WHERE {}
        "#,
        where_clause
    );

    let mut filtered_summary_query_exec =
        sqlx::query_as::<_, FilteredSummaryRow>(&filtered_summary_query);
    for param in &params {
        filtered_summary_query_exec = filtered_summary_query_exec.bind(param);
    }
    let filtered_result = filtered_summary_query_exec
        .fetch_one(&state.pool)
        .await
        .map_err(|err| super::db_error(err, "Unable to calculate filtered summary"))?;

    let filtered_summary = BudgetSummary {
        total_income: filtered_result.total_income,
        total_expenses: filtered_result.total_expenses,
        balance: filtered_result.total_income - filtered_result.total_expenses,
    };

    if filters.filtered {
        let template = DashboardSearchResultsTemplate {
            ctx,
            filtered_summary,
            filters,
            more_transactions,
            summary,
            transactions,
        };

        Ok(template
            .render()
            .map(axum::response::Html)
            .map_err(super::template_error)?
            .into_response())
    } else {
        let template = DashboardTemplate {
            ctx,
            accounts,
            categories,
            filtered_summary,
            filters,
            more_transactions,
            summary,
            transactions,
            user,
        };

        Ok(template
            .render()
            .map(axum::response::Html)
            .map_err(super::template_error)?
            .into_response())
    }
}

async fn calculate_summary(pool: &SqlitePool, user_id: i64) -> Result<BudgetSummary, sqlx::Error> {
    let result = sqlx::query_as::<_, SummaryRow>(
        r#"
        SELECT 
            CAST(COALESCE(SUM(CASE WHEN t.type = 'income' THEN t.amount ELSE 0 END), 0.0) AS REAL) as total_income,
            CAST(COALESCE(SUM(CASE WHEN t.type = 'expense' THEN t.amount ELSE 0 END), 0.0) AS REAL) as total_expenses
        FROM transactions t
        LEFT JOIN user_accounts ua ON t.account_id = ua.account_id AND ua.user_id = ?
        WHERE t.user_id = ? 
          AND (t.account_id IS NULL OR ua.is_mine = 1)
        "#,
    )
    .bind(user_id)
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    Ok(BudgetSummary {
        total_income: result.total_income,
        total_expenses: result.total_expenses,
        balance: result.total_income - result.total_expenses,
    })
}
