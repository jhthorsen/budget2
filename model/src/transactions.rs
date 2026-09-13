use super::Pool;
use chrono::{Months, NaiveDate};

/// Aggregated dashboard data for one user and one set of transaction filters.
#[derive(Debug, Default)]
pub struct Dashboard {
    /// Sum of amounts for matching income transactions.
    pub income: f64,
    /// Sum of amounts for matching expense transactions.
    pub expenses: f64,
    /// Number of transactions matching the supplied filters.
    pub transaction_count: i64,
    /// Number of matching transactions whose category is unset.
    pub uncategorized_count: i64,
    /// Accounts available in the transaction filter for this user.
    pub transaction_filter_accounts: Vec<TransactionFilterOption>,
    /// Categories available in the transaction filter, including uncategorized when present.
    pub transaction_filter_categories: Vec<TransactionFilterOption>,
    /// Daily totals grouped by account or category and transaction type.
    pub line_points: Vec<LinePoint>,
}

impl Dashboard {
    pub fn formatted_income(&self) -> String {
        format_amount(self.income)
    }

    pub fn formatted_expenses(&self) -> String {
        format_amount(self.expenses)
    }
}

/// A transaction row prepared for display in the dashboard table.
#[derive(Debug, sqlx::FromRow)]
pub struct Transaction {
    /// Date on which the transaction was processed.
    pub processed_at: String,
    /// Original transaction description.
    pub description: String,
    /// Friendly account name, falling back to the account name.
    pub account_name: String,
    /// Category name, or “Uncategorized” when no category is assigned.
    pub category_name: String,
    /// Transaction kind, currently `income` or `expense`.
    pub transaction_type: String,
    /// Amount in the account’s currency.
    pub amount: f64,
}

impl Transaction {
    pub fn formatted_amount(&self) -> String {
        format_amount(self.amount)
    }
}

/// One page of dashboard transactions and its pagination state.
#[derive(Debug)]
pub struct Transactions {
    /// Transactions returned for the requested page.
    pub transactions: Vec<Transaction>,
    /// True when the query found at least one additional transaction.
    pub has_more: bool,
}

/// An account or category choice for a transaction filter.
#[derive(Debug, sqlx::FromRow)]
pub struct TransactionFilterOption {
    /// Database identifier submitted when this option is selected.
    pub id: i64,
    /// Label shown in the filter dropdown.
    pub name: String,
}

/// One aggregated value sent to the dashboard’s daily movement chart.
#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct LinePoint {
    /// Calendar day represented by this aggregate.
    pub day: String,
    /// Chart series name: account when unfiltered by account, otherwise category.
    pub series: String,
    /// Aggregate amount for this day, series, and transaction type.
    pub amount: f64,
    /// Transaction kind used to draw the income or expense line.
    pub transaction_type: String,
    /// Distinct source currencies found in the grouped transactions; currently not displayed by the dashboard.
    pub original_currency: String,
}

/// Optional criteria applied to dashboard totals, chart data, and table rows.
#[derive(Debug, Default, Clone, Copy)]
pub struct TransactionFilters<'a> {
    /// Optional month/day prefix matched against `processed_at`.
    pub date: Option<&'a str>,
    /// Optional case-insensitive substring matched against `description`.
    pub description: Option<&'a str>,
    /// Optional account identifier.
    pub account_id: Option<i64>,
    /// Optional category identifier; `0` means no category.
    pub category_id: Option<i64>,
    /// Optional inclusive lower bound for `amount`.
    pub minimum_amount: Option<f64>,
}

impl Dashboard {
    pub async fn report(
        pool: &Pool,
        household_id: i64,
        viewer_id: i64,
        role: super::Role,
        filters: TransactionFilters<'_>,
    ) -> Result<Self, sqlx::Error> {
        let report_month = latest_report_month(pool, household_id, viewer_id, role).await?;

        let (transaction_filter_accounts, transaction_filter_categories) =
            Self::transaction_filter_options(pool, household_id, viewer_id, role).await?;
        let line_points = Self::daily_line_points(
            pool,
            household_id,
            viewer_id,
            role,
            &report_month,
            filters.date,
            filters.account_id,
        )
        .await?;
        let (income, expenses, transaction_count, uncategorized_count) =
            sqlx::query_as::<_, (f64, f64, i64, i64)>(
            r#"select
              coalesce(sum(case when t.type = 'income' then t.amount else 0.0 end), 0.0) as income,
              coalesce(sum(case when t.type = 'expense' then t.amount else 0.0 end), 0.0) as expenses,
              count(*) as transaction_count,
              coalesce(sum(case when t.category_id is null then 1 else 0 end), 0) as uncategorized_count
            from transactions t join accounts a on a.id = t.account_id
            where a.household_id = ?
              and (? = 'manager' or (? = 'assistant' and t.imported_by_user_id = ?) or (? = 'member' and a.user_id = ?))
              and (? is null or t.processed_at like ? || '%')
              and (? is null or instr(lower(t.description), lower(?)) > 0)
              and (? is null or t.account_id = ?)
              and (? is null or (? = 0 and t.category_id is null) or t.category_id = ?)
              and (? is null or t.amount >= ?)"#,
        )
            .bind(household_id)
            .bind(role.as_str())
            .bind(role.as_str())
            .bind(viewer_id)
            .bind(role.as_str())
            .bind(viewer_id)
            .bind(filters.date)
            .bind(filters.date)
            .bind(filters.description)
            .bind(filters.description)
            .bind(filters.account_id)
            .bind(filters.account_id)
            .bind(filters.category_id)
            .bind(filters.category_id)
            .bind(filters.category_id)
            .bind(filters.minimum_amount)
            .bind(filters.minimum_amount)
            .fetch_one(pool)
            .await?;

        Ok(Self {
            income,
            expenses,
            transaction_count,
            uncategorized_count,
            transaction_filter_accounts,
            transaction_filter_categories,
            line_points,
        })
    }

    async fn transaction_filter_options(
        pool: &Pool,
        household_id: i64,
        viewer_id: i64,
        role: super::Role,
    ) -> Result<(Vec<TransactionFilterOption>, Vec<TransactionFilterOption>), sqlx::Error> {
        let accounts = sqlx::query_as::<_, TransactionFilterOption>(
            r#"select id, coalesce(nullif(friendly, ''), name) as name
            from accounts where household_id = ? and (? != 'member' or user_id = ?) order by name"#,
        )
        .bind(household_id)
        .bind(role.as_str())
        .bind(viewer_id)
        .fetch_all(pool)
        .await?;
        let categories = sqlx::query_as::<_, TransactionFilterOption>(
            r#"select distinct c.id, c.name from categories c
            join transactions t on t.category_id = c.id join accounts a on a.id = t.account_id
            where a.household_id = ? and (? = 'manager' or (? = 'assistant' and t.imported_by_user_id = ?) or (? = 'member' and a.user_id = ?))
            union all select 0 as id, 'Uncategorized' as name
            where exists (select 1 from transactions t join accounts a on a.id = t.account_id
                where a.household_id = ? and (? = 'manager' or (? = 'assistant' and t.imported_by_user_id = ?) or (? = 'member' and a.user_id = ?)) and t.category_id is null)
            order by name"#,
        )
        .bind(household_id).bind(role.as_str()).bind(role.as_str()).bind(viewer_id).bind(role.as_str()).bind(viewer_id)
        .bind(household_id).bind(role.as_str()).bind(role.as_str()).bind(viewer_id).bind(role.as_str()).bind(viewer_id)
        .fetch_all(pool)
        .await?;
        Ok((accounts, categories))
    }

    async fn daily_line_points(
        pool: &Pool,
        household_id: i64,
        viewer_id: i64,
        role: super::Role,
        report_month: &str,
        date_filter: Option<&str>,
        account_id: Option<i64>,
    ) -> Result<Vec<LinePoint>, sqlx::Error> {
        let (start, end) = chart_interval(report_month, date_filter);
        if let Some(account_id) = account_id {
            sqlx::query_as::<_, LinePoint>(
                r#"select t.processed_at as day, coalesce(c.name, 'Uncategorized') as series,
                t.type as transaction_type, coalesce(group_concat(distinct nullif(trim(t.original_currency), '')), '') as original_currency,
                sum(t.amount) as amount from transactions t left join categories c on c.id = t.category_id
                join accounts a on a.id = t.account_id
                where a.household_id = ? and (? = 'manager' or (? = 'assistant' and t.imported_by_user_id = ?) or (? = 'member' and a.user_id = ?)) and t.account_id = ? and t.processed_at >= ? and t.processed_at < ?
                group by t.processed_at, t.category_id, t.type order by day, series, transaction_type"#,
            )
            .bind(household_id).bind(role.as_str()).bind(role.as_str()).bind(viewer_id).bind(role.as_str()).bind(viewer_id).bind(account_id).bind(&start).bind(&end)
            .fetch_all(pool).await
        } else {
            sqlx::query_as::<_, LinePoint>(
                r#"select t.processed_at as day, coalesce(nullif(a.friendly, ''), a.name) as series,
                t.type as transaction_type, coalesce(group_concat(distinct nullif(trim(t.original_currency), '')), '') as original_currency,
                sum(t.amount) as amount from transactions t join accounts a on a.id = t.account_id
                where a.household_id = ? and (? = 'manager' or (? = 'assistant' and t.imported_by_user_id = ?) or (? = 'member' and a.user_id = ?)) and t.processed_at >= ? and t.processed_at < ?
                group by t.processed_at, a.id, t.type order by day, series, transaction_type"#,
            )
            .bind(household_id).bind(role.as_str()).bind(role.as_str()).bind(viewer_id).bind(role.as_str()).bind(viewer_id).bind(&start).bind(&end)
            .fetch_all(pool).await
        }
    }

    pub async fn transactions(
        pool: &Pool,
        household_id: i64,
        viewer_id: i64,
        role: super::Role,
        filters: TransactionFilters<'_>,
        offset: i64,
        limit: i64,
    ) -> Result<Transactions, sqlx::Error> {
        let mut transactions = sqlx::query_as::<_, Transaction>(
            r#"select
              t.processed_at,
              t.description,
              coalesce(nullif(a.friendly, ''), a.name) as account_name,
              coalesce(c.name, 'Uncategorized') as category_name,
              t.type as transaction_type,
              t.amount
            from transactions t
            join accounts a on a.id = t.account_id
            left join categories c on c.id = t.category_id
            where a.household_id = ? and (? = 'manager' or (? = 'assistant' and t.imported_by_user_id = ?) or (? = 'member' and a.user_id = ?))
              and (? is null or t.processed_at like ? || '%')
              and (? is null or instr(lower(t.description), lower(?)) > 0)
              and (? is null or t.account_id = ?)
              and (? is null or (? = 0 and t.category_id is null) or t.category_id = ?)
              and (? is null or t.amount >= ?)
            order by t.processed_at desc, t.id desc
            limit ? offset ?"#,
        )
        .bind(household_id)
        .bind(role.as_str())
        .bind(role.as_str())
        .bind(viewer_id)
        .bind(role.as_str())
        .bind(viewer_id)
        .bind(filters.date)
        .bind(filters.date)
        .bind(filters.description)
        .bind(filters.description)
        .bind(filters.account_id)
        .bind(filters.account_id)
        .bind(filters.category_id)
        .bind(filters.category_id)
        .bind(filters.category_id)
        .bind(filters.minimum_amount)
        .bind(filters.minimum_amount)
        .bind(limit + 1)
        .bind(offset)
        .fetch_all(pool)
        .await?;
        let has_more = transactions.len() as i64 > limit;
        if has_more {
            transactions.pop();
        }
        Ok(Transactions {
            transactions,
            has_more,
        })
    }
}

async fn latest_report_month(
    pool: &Pool,
    household_id: i64,
    viewer_id: i64,
    role: super::Role,
) -> Result<String, sqlx::Error> {
    Ok(sqlx::query_scalar::<_, Option<String>>(
        "select max(substr(t.processed_at, 1, 7)) from transactions t join accounts a on a.id = t.account_id where a.household_id = ? and (? = 'manager' or (? = 'assistant' and t.imported_by_user_id = ?) or (? = 'member' and a.user_id = ?))",
    )
    .bind(household_id).bind(role.as_str()).bind(role.as_str()).bind(viewer_id).bind(role.as_str()).bind(viewer_id)
    .fetch_one(pool)
    .await?
    .unwrap_or_default())
}

fn chart_interval(report_month: &str, date_filter: Option<&str>) -> (String, String) {
    let month_start =
        |month: &str| NaiveDate::parse_from_str(&format!("{month}-01"), "%Y-%m-%d").ok();
    if let Some(date) = date_filter {
        if date.len() == 7 {
            if let Some(start) = month_start(date) {
                if let Some(end) = start.checked_add_months(Months::new(1)) {
                    return (start.to_string(), end.to_string());
                }
            }
        } else if date.len() == 10 {
            if let Ok(end) = NaiveDate::parse_from_str(date, "%Y-%m-%d") {
                if let (Some(start), Some(after)) =
                    (end.checked_sub_months(Months::new(12)), end.succ_opt())
                {
                    return (start.to_string(), after.to_string());
                }
            }
        }
    }
    if let Some(start) = month_start(report_month) {
        if let (Some(from), Some(to)) = (
            start.checked_sub_months(Months::new(11)),
            start.checked_add_months(Months::new(1)),
        ) {
            return (from.to_string(), to.to_string());
        }
    }
    (String::new(), String::new())
}

fn format_amount(amount: f64) -> String {
    let amount = format!("{:.0}", amount.abs());
    let mut formatted = String::with_capacity(amount.len() + amount.len() / 3);
    for (index, character) in amount.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            formatted.push(',');
        }
        formatted.push(character);
    }
    formatted.chars().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn chart_currencies_are_optional_and_aggregated_in_both_views() {
        let pool = crate::build_pool("sqlite::memory:", true).await.unwrap();
        for statement in [
            "insert into users (id, email, name, oauth_provider, oauth_id) values (1, 'chart@example.com', 'Chart', 'test', 'chart')",
            "insert into accounts (id, user_id, name) values (1, 1, 'Checking')",
            "insert into categories (id, name) values (1, 'Travel')",
            "insert into transactions (user_id, account_id, category_id, type, amount, original_amount, original_currency, description, processed_at) values
             (1, 1, 1, 'expense', 12.34, 10, 'USD', 'A', '2026-01-01'),
             (1, 1, 1, 'expense', 10, 10, 'USD', 'B', '2026-01-01'),
             (1, 1, 1, 'income', 5, 5, 'EUR', 'C', '2026-01-01'),
             (1, 1, 1, 'expense', 1, 1, '', 'D', '2026-01-02')",
        ] {
            sqlx::query(statement).execute(&pool).await.unwrap();
        }
        for account_id in [None, Some(1)] {
            let report = Dashboard::report(
                &pool,
                1,
                1,
                crate::Role::Manager,
                TransactionFilters {
                    account_id,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
            assert_eq!(report.line_points.len(), 3);
            assert!((report.line_points[0].amount - 22.34).abs() < 0.00001);
            assert_eq!(report.line_points[0].transaction_type, "expense");
            assert_eq!(report.line_points[0].original_currency, "USD");
            assert_eq!(report.line_points[1].amount, 5.0);
            assert_eq!(report.line_points[1].transaction_type, "income");
            assert_eq!(report.line_points[1].original_currency, "EUR");
            assert!(report.line_points[2].original_currency.is_empty());
        }
    }
}
