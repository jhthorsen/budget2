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

    // Fetch chart data - group by account if single category selected, otherwise by category
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

fn assign_color(category_color: Option<String>, index: usize, is_income: bool) -> String {
    if let Some(color) = category_color
        && !color.is_empty()
    {
        return color;
    }

    let palette = palette();
    if is_income {
        // Use green tones for income
        "oklch(68% 0.18 145)".to_string()
    } else {
        // Use palette colors for expenses
        palette[index % palette.len()].to_string()
    }
}

async fn fetch_chart_data(
    pool: &SqlitePool,
    transaction: &Transaction,
) -> Result<ChartData, sqlx::Error> {
    let group_by_account = transaction.category_id.unwrap_or_default() > 0;
    let raw_data = if group_by_account {
        transaction.chart_data_by_account(pool).await?
    } else {
        transaction.chart_data_by_category(pool).await?
    };

    let days_in_month = filters::days_in_month(&transaction.transaction_date).unwrap_or(30);

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
        all_categories.push(("Other".to_string(), palette()[10].to_string()));
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
                    ("Other".to_string(), palette()[10].to_string())
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
