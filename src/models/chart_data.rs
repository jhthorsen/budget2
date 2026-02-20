use super::transactions::TransactionQuery;
use super::{Pool, Serialize};
use std::collections::HashMap;

#[derive(Debug, Default, Serialize)]
pub struct ChartData {
    pub by_day: HashMap<String, Vec<DayData>>,
    pub legends: HashMap<String, String>,
}

impl ChartData {
    pub fn color(&self, legend: &str) -> &str {
        self.legends
            .get(legend)
            .map(|color| color.as_str())
            .unwrap_or("gray")
    }

    pub fn x_labels(&self) -> Vec<(usize, &str)> {
        let mut dates = self.by_day.keys().collect::<Vec<_>>();
        dates.sort();

        let n = dates.len() / 12;
        let mut labels: Vec<(usize, &str)> = vec![];
        for (i, date) in dates.iter().enumerate() {
            if i > 0 && i % n == 0 {
                labels.push((i, date.as_str()));
            }
        }

        labels
    }

    pub async fn by_account(
        pool: &Pool,
        query: &TransactionQuery,
    ) -> Result<Vec<DayData>, sqlx::Error> {
        let description = query.description_like_pattern();
        sqlx::query_as!(
            DayData,
            r#"select
              t.account_id as 'group_id!',
              '' as color,
              t.type as transaction_type,
              coalesce(a.name, 'No account') as group_name,
              strftime('%Y-%m-%d', t.processed_at) as 'date!',
              cast(sum(t.amount) as real) as amount
            from transactions t
              left join accounts a on t.account_id = a.id
            where (length(?) = 0 or t.type = ?)
              and (? <= 0 or t.category_id = ?)
              and (? = -1 or t.category_id = null)
              and (length(?) = 0 or t.description like ?)
              and (length(?) != 7 or strftime('%Y-%m', t.processed_at) = ?)
              and (length(?) != 4 or strftime('%Y', t.processed_at) = ?)
            group by processed_at, account_id, transaction_type
            order by processed_at, transaction_type, amount desc"#,
            query.transaction_type,
            query.transaction_type,
            query.category_id,
            query.category_id,
            query.category_id,
            description,
            description,
            query.processed_at,
            query.processed_at,
            query.processed_at,
            query.processed_at,
        )
        .fetch_all(pool)
        .await
    }

    pub async fn by_category(
        pool: &Pool,
        query: &TransactionQuery,
    ) -> Result<Vec<DayData>, sqlx::Error> {
        let description = query.description_like_pattern();
        sqlx::query_as!(
            DayData,
            r#"select
              t.category_id as 'group_id!',
              t.type as transaction_type,
              '' as color,
              coalesce(c.name, 'No category') as group_name,
              strftime('%Y-%m-%d', t.processed_at) as 'date!',
              cast(sum(t.amount) as real) as amount
            from transactions t
            left join categories c on t.category_id = c.id
            where (length(?) = 0 or t.type = ?)
              and (? <= 0 or t.account_id = ?)
              and (length(?) = 0 or t.description like ?)
              and (length(?) != 7 or strftime('%Y-%m', t.processed_at) = ?)
              and (length(?) != 4 or strftime('%Y', t.processed_at) = ?)
            group by processed_at, category_id, transaction_type
            order by processed_at, transaction_type, amount desc"#,
            query.transaction_type,
            query.transaction_type,
            query.account_id,
            query.account_id,
            description,
            description,
            query.processed_at,
            query.processed_at,
            query.processed_at,
            query.processed_at,
        )
        .fetch_all(pool)
        .await
    }
}

#[derive(Clone, Debug, sqlx::FromRow, Serialize)]
pub struct DayData {
    pub date: String,
    pub amount: f64,
    pub color: String,
    pub group_id: i64,
    pub group_name: String,
    pub transaction_type: String,
}
