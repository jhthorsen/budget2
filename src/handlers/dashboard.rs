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

    // Fetch chart data - group by account if single category selected, otherwise by category
    let group_by_account = filters.category_id > 0;
    let chart_data = fetch_chart_data(
        &state.pool,
        &where_clause,
        &params,
        &filters.month,
        group_by_account,
    )
    .await
    .map_err(|err| super::db_error(err, "Unable to fetch chart data"))?;

    if filters.filtered {
        let template = DashboardSearchResultsTemplate {
            ctx,
            chart_data,
            filters,
            more_transactions,
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
            chart_data,
            filters,
            more_transactions,
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

async fn fetch_chart_data(
    pool: &SqlitePool,
    where_clause: &str,
    params: &[String],
    month: &str,
    group_by_account: bool,
) -> Result<ChartData, sqlx::Error> {
    use std::collections::HashMap;

    // Professional color palette optimized for both light and dark modes
    // Using OKLCH color space for perceptually uniform colors
    fn get_professional_palette() -> Vec<&'static str> {
        vec![
            "oklch(65% 0.20 250)", // Blue
            "oklch(70% 0.19 145)", // Green
            "oklch(75% 0.20 50)",  // Orange
            "oklch(68% 0.20 320)", // Purple
            "oklch(72% 0.18 180)", // Cyan
            "oklch(70% 0.20 25)",  // Red-Orange
            "oklch(65% 0.15 280)", // Indigo
            "oklch(73% 0.17 85)",  // Yellow-Green
            "oklch(68% 0.18 350)", // Magenta
            "oklch(70% 0.16 200)", // Sky Blue
            "oklch(60% 0.10 270)", // Other (muted purple-gray)
        ]
    }

    fn assign_color(category_color: Option<String>, index: usize, is_income: bool) -> String {
        if let Some(color) = category_color {
            if !color.is_empty() {
                return color;
            }
        }

        let palette = get_professional_palette();
        if is_income {
            // Use green tones for income
            "oklch(68% 0.18 145)".to_string()
        } else {
            // Use palette colors for expenses
            palette[index % palette.len()].to_string()
        }
    }

    #[derive(sqlx::FromRow)]
    struct DayData {
        day: i64,
        group_id: Option<i64>,
        group_name: Option<String>,
        group_color: Option<String>,
        transaction_type: String,
        total: f64,
    }

    // Query to get daily aggregates by category or account
    let query = if group_by_account {
        format!(
            r#"
            SELECT
                CAST(strftime('%d', t.transaction_date) AS INTEGER) as day,
                t.account_id as group_id,
                COALESCE(a.name, 'No Account') as group_name,
                NULL as group_color,
                t.type as transaction_type,
                CAST(SUM(t.amount) AS REAL) as total
            FROM transactions t
            LEFT JOIN accounts a ON t.account_id = a.id
            WHERE {}
            GROUP BY day, t.account_id, t.type
            ORDER BY day, t.type, total DESC
            "#,
            where_clause
        )
    } else {
        format!(
            r#"
            SELECT
                CAST(strftime('%d', t.transaction_date) AS INTEGER) as day,
                t.category_id as group_id,
                COALESCE(c.name, 'Uncategorized') as group_name,
                c.color as group_color,
                t.type as transaction_type,
                CAST(SUM(t.amount) AS REAL) as total
            FROM transactions t
            LEFT JOIN categories c ON t.category_id = c.id
            WHERE {}
            GROUP BY day, t.category_id, t.type
            ORDER BY day, t.type, total DESC
            "#,
            where_clause
        )
    };

    let mut query_exec = sqlx::query_as::<_, DayData>(&query);
    for param in params {
        query_exec = query_exec.bind(param);
    }
    let raw_data = query_exec.fetch_all(pool).await?;

    // Determine number of days in the month
    let days_in_month = if !month.is_empty() {
        let parts: Vec<&str> = month.split('-').collect();
        if parts.len() == 2 {
            if let (Ok(year), Ok(month_num)) = (parts[0].parse::<i32>(), parts[1].parse::<u32>()) {
                chrono::NaiveDate::from_ymd_opt(year, month_num, 1)
                    .and_then(|d| {
                        if month_num == 12 {
                            chrono::NaiveDate::from_ymd_opt(year + 1, 1, 1)
                        } else {
                            chrono::NaiveDate::from_ymd_opt(year, month_num + 1, 1)
                        }
                        .map(|next| (next - d).num_days() as i64)
                    })
                    .unwrap_or(31)
            } else {
                31
            }
        } else {
            31
        }
    } else {
        31
    };

    // Group to find top 10 expenses
    let mut group_totals: HashMap<(Option<i64>, String, Option<String>), f64> = HashMap::new();
    for row in &raw_data {
        if row.transaction_type == "expense" {
            let key = (
                row.group_id,
                row.group_name.clone().unwrap_or_else(|| {
                    if group_by_account {
                        "No Account"
                    } else {
                        "Uncategorized"
                    }
                    .to_string()
                }),
                row.group_color.clone(),
            );
            *group_totals.entry(key).or_insert(0.0) += row.total;
        }
    }

    let mut group_vec: Vec<_> = group_totals.into_iter().collect();
    group_vec.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Assign colors to top 10
    let top_groups: HashMap<(Option<i64>, String), String> = group_vec
        .iter()
        .take(10)
        .enumerate()
        .map(|(idx, ((id, name, db_color), _))| {
            let color = assign_color(db_color.clone(), idx, false);
            ((*id, name.clone()), color)
        })
        .collect();

    // Build legend (all unique groups in the data)
    let mut all_categories: Vec<(String, String)> = top_groups
        .iter()
        .map(|((_, name), color)| (name.clone(), color.clone()))
        .collect();
    all_categories.sort_by(|a, b| a.0.cmp(&b.0));
    if group_vec.len() > 10 {
        all_categories.push((
            "Other".to_string(),
            get_professional_palette()[10].to_string(),
        ));
    }

    // Process raw data into day stacks
    let mut days = Vec::new();
    let mut max_income = 0.0_f64;
    let mut max_expenses = 0.0_f64;

    for day_num in 1..=days_in_month {
        let mut income_map: HashMap<String, CategoryStack> = HashMap::new();
        let mut expense_map: HashMap<String, CategoryStack> = HashMap::new();

        for row in raw_data.iter().filter(|r| r.day == day_num) {
            let category_key = (
                row.group_id,
                row.group_name
                    .clone()
                    .unwrap_or_else(|| "Uncategorized".to_string()),
            );

            let (name, color) = if row.transaction_type == "expense" {
                if let Some(color) = top_groups.get(&category_key) {
                    (category_key.1.clone(), color.clone())
                } else {
                    (
                        "Other".to_string(),
                        get_professional_palette()[10].to_string(),
                    )
                }
            } else {
                (
                    row.group_name
                        .clone()
                        .unwrap_or_else(|| "Uncategorized".to_string()),
                    assign_color(row.group_color.clone(), 0, true),
                )
            };

            let stack = CategoryStack {
                category_id: row.group_id,
                category_name: name.clone(),
                category_color: color.clone(),
                amount: row.total,
                y_pos: 0,  // Will be calculated later
                height: 0, // Will be calculated later
            };

            if row.transaction_type == "income" {
                income_map
                    .entry(name)
                    .and_modify(|s| s.amount += row.total)
                    .or_insert(stack);
            } else {
                expense_map
                    .entry(name)
                    .and_modify(|s| s.amount += row.total)
                    .or_insert(stack);
            }
        }

        let mut income_stacks: Vec<_> = income_map.into_values().collect();
        let mut expense_stacks: Vec<_> = expense_map.into_values().collect();

        income_stacks.sort_by(|a, b| {
            b.amount
                .partial_cmp(&a.amount)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        expense_stacks.sort_by(|a, b| {
            b.amount
                .partial_cmp(&a.amount)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let total_income: f64 = income_stacks.iter().map(|s| s.amount).sum();
        let total_expenses: f64 = expense_stacks.iter().map(|s| s.amount).sum();

        max_income = max_income.max(total_income);
        max_expenses = max_expenses.max(total_expenses);

        days.push(DayStack {
            day: day_num,
            income_stacks,
            expense_stacks,
            total_income,
            total_expenses,
        });
    }

    // Second pass: calculate positions now that we know max values
    const ZERO_Y: i32 = 180;
    const MAX_HEIGHT: f64 = 160.0;

    for day in &mut days {
        let mut y_pos = ZERO_Y;
        for stack in &mut day.expense_stacks {
            let height = if max_expenses > 0.0 {
                (stack.amount * MAX_HEIGHT / max_expenses) as i32
            } else {
                0
            };
            stack.y_pos = y_pos;
            stack.height = height;
            y_pos += height;
        }

        let mut y_pos = ZERO_Y;
        for stack in &mut day.income_stacks {
            let height = if max_income > 0.0 {
                (stack.amount * MAX_HEIGHT / max_income) as i32
            } else {
                0
            };
            y_pos -= height;
            stack.y_pos = y_pos;
            stack.height = height;
        }
    }

    // Calculate totals and pie chart data
    let mut income_totals: HashMap<String, (f64, String)> = HashMap::new();
    let mut expense_totals: HashMap<String, (f64, String)> = HashMap::new();
    let mut total_income = 0.0;
    let mut total_expenses = 0.0;

    for day in &days {
        for stack in &day.income_stacks {
            let entry = income_totals
                .entry(stack.category_name.clone())
                .or_insert((0.0, stack.category_color.clone()));
            entry.0 += stack.amount;
            total_income += stack.amount;
        }
        for stack in &day.expense_stacks {
            let entry = expense_totals
                .entry(stack.category_name.clone())
                .or_insert((0.0, stack.category_color.clone()));
            entry.0 += stack.amount;
            total_expenses += stack.amount;
        }
    }

    // Create pie slices for income
    let mut income_pie_slices = Vec::new();
    let mut current_angle = -90.0;
    const CENTER: i32 = 150;
    const RADIUS: i32 = 100;

    for (name, (amount, color)) in income_totals.iter() {
        if total_income > 0.0 {
            let percentage = amount / total_income;
            let angle = percentage * 360.0;
            let end_angle = current_angle + angle;

            let start_rad = current_angle * std::f64::consts::PI / 180.0;
            let end_rad = end_angle * std::f64::consts::PI / 180.0;
            let start_x = CENTER + (RADIUS as f64 * start_rad.cos()) as i32;
            let start_y = CENTER + (RADIUS as f64 * start_rad.sin()) as i32;
            let end_x = CENTER + (RADIUS as f64 * end_rad.cos()) as i32;
            let end_y = CENTER + (RADIUS as f64 * end_rad.sin()) as i32;
            let large_arc = if angle > 180.0 { 1 } else { 0 };

            income_pie_slices.push(PieSlice {
                name: name.clone(),
                color: color.clone(),
                amount: *amount,
                percentage: (percentage * 100.0) as i32,
                start_x,
                start_y,
                end_x,
                end_y,
                large_arc,
            });

            current_angle = end_angle;
        }
    }

    // Create pie slices for expenses
    let mut expense_pie_slices = Vec::new();
    let mut current_angle = -90.0;
    for (name, (amount, color)) in expense_totals.iter() {
        if total_expenses > 0.0 {
            let percentage = amount / total_expenses;
            let angle = percentage * 360.0;
            let end_angle = current_angle + angle;

            let start_rad = current_angle * std::f64::consts::PI / 180.0;
            let end_rad = end_angle * std::f64::consts::PI / 180.0;
            let start_x = CENTER + (RADIUS as f64 * start_rad.cos()) as i32;
            let start_y = CENTER + (RADIUS as f64 * start_rad.sin()) as i32;
            let end_x = CENTER + (RADIUS as f64 * end_rad.cos()) as i32;
            let end_y = CENTER + (RADIUS as f64 * end_rad.sin()) as i32;
            let large_arc = if angle > 180.0 { 1 } else { 0 };

            expense_pie_slices.push(PieSlice {
                name: name.clone(),
                color: color.clone(),
                amount: *amount,
                percentage: (percentage * 100.0) as i32,
                start_x,
                start_y,
                end_x,
                end_y,
                large_arc,
            });

            current_angle = end_angle;
        }
    }

    Ok(ChartData {
        days,
        max_income,
        max_expenses,
        total_income,
        total_expenses,
        all_categories,
        income_pie_slices,
        expense_pie_slices,
    })
}
