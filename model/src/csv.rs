use crate::{ImportRule, Pool, User};
use csv::{ReaderBuilder, StringRecord};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::Path};

#[derive(Debug, Deserialize)]
pub struct ColumnMapping {
    pub file_id: String,
    pub date_column: String,
    pub income_column: String,
    pub expense_column: String,
    pub description_column: String,
    pub account_column: Option<String>,
    pub account_fixed_name: Option<String>,
    pub category_column: Option<String>,
    pub currency_multiplier: Option<String>,
    #[serde(skip)]
    header_map: HashMap<String, usize>,
}

impl ColumnMapping {
    fn with_header_map(self, header_map: HashMap<String, usize>) -> Self {
        Self { header_map, ..self }
    }

    fn value<'a>(&self, record: &'a StringRecord, column: &str) -> Result<&'a str, String> {
        let column = column.trim();
        if column.is_empty() {
            return Err("No CSV column selected".to_string());
        }
        self.header_map
            .get(column)
            .and_then(|index| record.get(*index))
            .ok_or_else(|| format!("Column {column} not found"))
    }

    fn optional_value<'a>(
        &self,
        record: &'a StringRecord,
        column: Option<&str>,
    ) -> Option<&'a str> {
        column
            .filter(|column| !column.trim().is_empty())
            .and_then(|column| self.value(record, column).ok())
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }
}

#[derive(Debug, Serialize)]
pub struct ImportError {
    pub row_number: usize,
    pub row_data: String,
    pub error: String,
}

#[derive(Debug, Serialize)]
pub struct ImportResult {
    pub total_rows: usize,
    pub successful: usize,
    pub errors: Vec<ImportError>,
}

pub fn read_csv_headers(path: &Path) -> Result<Vec<String>, String> {
    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .from_path(path)
        .map_err(|err| err.to_string())?;
    reader
        .headers()
        .map(|headers| headers.iter().map(str::to_owned).collect())
        .map_err(|err| err.to_string())
}

pub async fn import_csv_file(
    pool: &Pool,
    user: &User,
    path: &Path,
    mapping: ColumnMapping,
) -> Result<ImportResult, String> {
    if mapping.date_column.trim().is_empty()
        || mapping.description_column.trim().is_empty()
        || (mapping.income_column.trim().is_empty() && mapping.expense_column.trim().is_empty())
    {
        return Err("Date, description, and at least one amount column are required".to_string());
    }

    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .from_path(path)
        .map_err(|err| err.to_string())?;
    let headers = reader.headers().map_err(|err| err.to_string())?.clone();
    let mapping = mapping.with_header_map(
        headers
            .iter()
            .enumerate()
            .map(|(index, header)| (header.to_owned(), index))
            .collect(),
    );
    let rules = ImportRule::all(pool).await.map_err(|err| err.to_string())?;
    let multiplier = mapping
        .currency_multiplier
        .as_deref()
        .unwrap_or("1")
        .parse::<f64>()
        .map_err(|_| "Currency multiplier must be a number".to_string())?;

    let mut result = ImportResult {
        total_rows: 0,
        successful: 0,
        errors: Vec::new(),
    };
    for (index, record) in reader.records().enumerate() {
        let row_number = index + 2;
        result.total_rows += 1;
        let record = match record {
            Ok(record) => record,
            Err(err) => {
                result.errors.push(ImportError {
                    row_number,
                    row_data: "Unable to read row".to_string(),
                    error: err.to_string(),
                });
                continue;
            }
        };

        match import_row(pool, user, &record, &mapping, &rules, multiplier).await {
            Ok(true) => result.successful += 1,
            Ok(false) => result.errors.push(ImportError {
                row_number,
                row_data: format!("{record:?}"),
                error: "No valid amount found".to_string(),
            }),
            Err(error) => result.errors.push(ImportError {
                row_number,
                row_data: format!("{record:?}"),
                error,
            }),
        }
    }
    Ok(result)
}

async fn import_row(
    pool: &Pool,
    user: &User,
    record: &StringRecord,
    mapping: &ColumnMapping,
    rules: &[ImportRule],
    multiplier: f64,
) -> Result<bool, String> {
    let date = normalize_date(mapping.value(record, &mapping.date_column)?)?;
    let description = mapping.value(record, &mapping.description_column)?.trim();
    if description.is_empty() {
        return Err("Description is empty".to_string());
    }

    let account_name = mapping
        .optional_value(record, mapping.account_column.as_deref())
        .or_else(|| {
            mapping
                .account_fixed_name
                .as_deref()
                .map(str::trim)
                .filter(|name| !name.is_empty())
        });
    let mut account_id = if let Some(name) = account_name {
        account_id(pool, user.id, name).await?
    } else {
        0
    };
    let category_name = mapping.optional_value(record, mapping.category_column.as_deref());
    let mut category_id = if let Some(name) = category_name {
        Some(category_id(pool, name).await?)
    } else {
        None
    };

    for rule in rules {
        let matches_description = rule
            .match_description
            .as_deref()
            .is_some_and(|needle| description.to_lowercase().contains(&needle.to_lowercase()));
        let matches_account = rule.match_account.as_deref().is_some_and(|needle| {
            account_name.is_some_and(|name| name.to_lowercase().contains(&needle.to_lowercase()))
        });
        if matches_description || matches_account {
            if account_id == 0 {
                account_id = rule.account_id.unwrap_or_default();
            }
            if category_id.is_none() {
                category_id = rule.category_id;
            }
        }
    }
    if account_id == 0 {
        return Err("No account selected or matched by an import rule".to_string());
    }

    let mut imported = false;
    for (column, kind) in [
        (&mapping.income_column, "income"),
        (&mapping.expense_column, "expense"),
    ] {
        let Some(raw) = mapping.optional_value(record, Some(column)) else {
            continue;
        };
        let original_amount = normalize_amount(raw)?;
        let transaction_type = if original_amount >= 0.0 {
            kind
        } else if kind == "income" {
            "expense"
        } else {
            "income"
        };
        sqlx::query(
            "insert into transactions (user_id, account_id, category_id, type, amount, original_amount, description, source, processed_at) values (?, ?, ?, ?, ?, ?, ?, 'csv', ?)",
        )
        .bind(user.id)
        .bind(account_id)
        .bind(category_id)
        .bind(transaction_type)
        .bind((original_amount * multiplier).abs())
        .bind(original_amount)
        .bind(description)
        .bind(&date)
        .execute(pool)
        .await
        .map_err(|err| err.to_string())?;
        imported = true;
    }
    Ok(imported)
}

async fn account_id(pool: &Pool, user_id: i64, name: &str) -> Result<i64, String> {
    if let Some(id) =
        sqlx::query_scalar::<_, i64>("select id from accounts where user_id = ? and name = ?")
            .bind(user_id)
            .bind(name)
            .fetch_optional(pool)
            .await
            .map_err(|err| err.to_string())?
    {
        return Ok(id);
    }
    sqlx::query("insert into accounts (user_id, name, friendly) values (?, ?, ?)")
        .bind(user_id)
        .bind(name)
        .bind(name)
        .execute(pool)
        .await
        .map(|result| result.last_insert_rowid())
        .map_err(|err| err.to_string())
}

async fn category_id(pool: &Pool, name: &str) -> Result<i64, String> {
    if let Some(id) = sqlx::query_scalar::<_, i64>("select id from categories where name = ?")
        .bind(name)
        .fetch_optional(pool)
        .await
        .map_err(|err| err.to_string())?
    {
        return Ok(id);
    }
    sqlx::query("insert into categories (name) values (?)")
        .bind(name)
        .execute(pool)
        .await
        .map(|result| result.last_insert_rowid())
        .map_err(|err| err.to_string())
}

fn normalize_amount(value: &str) -> Result<f64, String> {
    value
        .trim()
        .replace([',', '$'], "")
        .parse::<f64>()
        .map_err(|_| format!("Invalid amount: {value}"))
}

fn normalize_date(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.len() == 10 && value.as_bytes().get(4) == Some(&b'-') {
        return Ok(value.to_string());
    }
    let parts: Vec<_> = value.split('/').collect();
    if parts.len() == 3 {
        return Ok(format!("{}-{:0>2}-{:0>2}", parts[0], parts[1], parts[2]));
    }
    Err(format!("Invalid date format: {value}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_common_csv_values() {
        assert_eq!(normalize_amount("$1,234.50").unwrap(), 1234.5);
        assert_eq!(normalize_date("2026/9/2").unwrap(), "2026-09-02");
        assert!(normalize_date("not-a-date").is_err());
    }
}
