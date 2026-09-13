use crate::{ImportRule, Pool, User};
use chrono::NaiveDate;
use csv::{ReaderBuilder, StringRecord};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::Path};

#[derive(Debug, Deserialize)]
pub struct ColumnMapping {
    pub file_id: String,
    pub date_column: String,
    pub date_format: Option<String>,
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

#[derive(Debug, Clone)]
pub struct DateFormatOption {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Default)]
pub struct ColumnSuggestions {
    pub date_column: String,
    pub description_column: String,
    pub income_column: String,
    pub expense_column: String,
    pub account_column: String,
    pub category_column: String,
    pub date_formats: Vec<DateFormatOption>,
}

pub fn read_csv_headers(path: &Path) -> Result<Vec<String>, String> {
    let delimiter = detect_delimiter(path)?;
    let mut reader = ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(true)
        .from_path(path)
        .map_err(|err| err.to_string())?;
    reader
        .headers()
        .map(|headers| headers.iter().map(str::to_owned).collect())
        .map_err(|err| err.to_string())
}

pub fn suggest_columns(path: &Path) -> Result<ColumnSuggestions, String> {
    let delimiter = detect_delimiter(path)?;
    let mut reader = ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(true)
        .from_path(path)
        .map_err(|err| err.to_string())?;
    let headers: Vec<String> = reader
        .headers()
        .map_err(|err| err.to_string())?
        .iter()
        .map(str::to_owned)
        .collect();
    let samples: Vec<StringRecord> = reader.records().take(25).filter_map(Result::ok).collect();

    let find = |names: &[&str]| {
        headers
            .iter()
            .find(|header| {
                let header = header.to_lowercase();
                names.iter().any(|name| header.contains(name))
            })
            .cloned()
            .unwrap_or_default()
    };
    let date_column = find(&["date", "dato", "processed", "booked"]);
    let date_formats = if date_column.is_empty() {
        Vec::new()
    } else {
        let index = headers
            .iter()
            .position(|header| header == &date_column)
            .unwrap();
        let values: Vec<_> = samples
            .iter()
            .filter_map(|record| record.get(index))
            .collect();
        let matching: Vec<_> = [
            ("ymd", "YYYY-MM-DD"),
            ("ydm", "YYYY-DD-MM"),
            ("dmy", "DD.MM.YYYY"),
            ("mdy", "MM.DD.YYYY"),
        ]
        .into_iter()
        .filter(|(format, _)| {
            values
                .iter()
                .all(|value| parse_date(value, Some(format)).is_ok())
        })
        .collect();
        if matching.len() > 1 {
            matching
                .into_iter()
                .map(|(value, label)| DateFormatOption {
                    value: value.to_string(),
                    label: label.to_string(),
                })
                .collect()
        } else {
            Vec::new()
        }
    };

    Ok(ColumnSuggestions {
        date_column,
        description_column: find(&["description", "forklaring", "beskrivelse", "memo", "text"]),
        income_column: find(&["income", "credit", "inntekt", "deposit", "inn ", "inn paa"]),
        expense_column: find(&["expense", "debit", "kostnad", "withdrawal", "ut ", "ut av"]),
        account_column: headers
            .iter()
            .find(|header| {
                let header = header.to_lowercase();
                (header.contains("account") || header == "konto")
                    && !header.contains("inn ")
                    && !header.contains("ut ")
            })
            .cloned()
            .unwrap_or_default(),
        category_column: find(&["category", "kategori"]),
        date_formats,
    })
}

pub async fn import_csv_file(
    pool: &Pool,
    user: &User,
    household_id: i64,
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
        .delimiter(detect_delimiter(path)?)
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
    let rules = ImportRule::all(pool, household_id)
        .await
        .map_err(|err| err.to_string())?;
    let multiplier = mapping
        .currency_multiplier
        .as_deref()
        .unwrap_or("1")
        .parse::<f64>()
        .map_err(|_| "Currency multiplier must be a number".to_string())?;
    if !multiplier.is_finite() {
        return Err("Currency multiplier must be finite".to_string());
    }

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

        match import_row(
            pool,
            user,
            household_id,
            &record,
            &mapping,
            &rules,
            multiplier,
        )
        .await
        {
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
    household_id: i64,
    record: &StringRecord,
    mapping: &ColumnMapping,
    rules: &[ImportRule],
    multiplier: f64,
) -> Result<bool, String> {
    let date = normalize_date(
        mapping.value(record, &mapping.date_column)?,
        mapping
            .date_format
            .as_deref()
            .filter(|format| !format.is_empty()),
    )?;
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
        account_id(pool, household_id, user.id, name).await?
    } else {
        0
    };
    let category_name = mapping.optional_value(record, mapping.category_column.as_deref());
    let mut category_id = if let Some(name) = category_name {
        Some(category_id(pool, household_id, name).await?)
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
            "insert into transactions (user_id, imported_by_user_id, account_id, category_id, type, amount, original_amount, description, source, processed_at) values (?, ?, ?, ?, ?, ?, ?, ?, 'csv', ?)",
        )
        .bind(user.id)
        .bind(user.id)
        .bind(account_id)
        .bind(category_id)
        .bind(transaction_type)
        .bind(scaled_amount(original_amount, multiplier)?)
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

async fn account_id(
    pool: &Pool,
    household_id: i64,
    user_id: i64,
    name: &str,
) -> Result<i64, String> {
    if let Some(id) =
        sqlx::query_scalar::<_, i64>("select id from accounts where household_id = ? and name = ?")
            .bind(household_id)
            .bind(name)
            .fetch_optional(pool)
            .await
            .map_err(|err| err.to_string())?
    {
        return Ok(id);
    }
    sqlx::query("insert into accounts (user_id, household_id, name, friendly) values (?, ?, ?, ?)")
        .bind(user_id)
        .bind(household_id)
        .bind(name)
        .bind(name)
        .execute(pool)
        .await
        .map(|result| result.last_insert_rowid())
        .map_err(|err| err.to_string())
}

async fn category_id(pool: &Pool, household_id: i64, name: &str) -> Result<i64, String> {
    if let Some(id) = sqlx::query_scalar::<_, i64>(
        "select id from categories where household_id = ? and name = ?",
    )
    .bind(household_id)
    .bind(name)
    .fetch_optional(pool)
    .await
    .map_err(|err| err.to_string())?
    {
        return Ok(id);
    }
    sqlx::query("insert into categories (household_id, name) values (?, ?)")
        .bind(household_id)
        .bind(name)
        .execute(pool)
        .await
        .map(|result| result.last_insert_rowid())
        .map_err(|err| err.to_string())
}

fn normalize_amount(value: &str) -> Result<f64, String> {
    let amount = value
        .trim()
        .replace([',', '$'], "")
        .parse::<f64>()
        .map_err(|_| format!("Invalid amount: {value}"))?;
    if amount.is_finite() {
        Ok(amount)
    } else {
        Err(format!("Invalid amount: {value}"))
    }
}

fn scaled_amount(amount: f64, multiplier: f64) -> Result<f64, String> {
    let scaled = (amount * multiplier).abs();
    scaled
        .is_finite()
        .then_some(scaled)
        .ok_or_else(|| "Amount is outside the supported range".to_string())
}

fn detect_delimiter(path: &Path) -> Result<u8, String> {
    let mut best = (b',', 0);
    for delimiter in [b',', b';', b'\t', b'|'] {
        let mut reader = ReaderBuilder::new()
            .delimiter(delimiter)
            .has_headers(true)
            .from_path(path)
            .map_err(|err| err.to_string())?;
        let columns = reader.headers().map_err(|err| err.to_string())?.len();
        if columns > best.1 {
            best = (delimiter, columns);
        }
    }
    Ok(best.0)
}

fn normalize_date(value: &str, format: Option<&str>) -> Result<String, String> {
    let value = value.trim();
    let parts: Vec<_> = value
        .split(|character: char| matches!(character, '.' | '/' | '-'))
        .collect();
    if parts.len() != 3 || parts.iter().any(|part| part.is_empty()) {
        return Err(format!("Invalid date format: {value}"));
    }

    let numbers: Vec<u32> = parts
        .iter()
        .map(|part| {
            part.parse()
                .map_err(|_| format!("Invalid date format: {value}"))
        })
        .collect::<Result<_, _>>()?;
    let (year, first, second, year_first) = if parts[0].len() == 4 {
        (numbers[0], numbers[1], numbers[2], true)
    } else if parts[2].len() == 4 {
        (numbers[2], numbers[0], numbers[1], false)
    } else {
        return Err(format!("Invalid date format: {value}"));
    };

    let (year, month, day) = if let Some(format) = format {
        match format {
            "ymd" => (numbers[0], numbers[1], numbers[2]),
            "ydm" => (numbers[0], numbers[2], numbers[1]),
            "dmy" => (numbers[2], numbers[1], numbers[0]),
            "mdy" => (numbers[2], numbers[0], numbers[1]),
            _ => return Err(format!("Unknown date format: {format}")),
        }
    } else {
        let (month, day) = if year_first {
            // YYYY-MM-DD is the default; YYYY-DD-MM is recognized when the
            // supposed month is greater than 12.
            if first > 12 && first <= 31 {
                (second, first)
            } else {
                (first, second)
            }
        } else {
            // DD.MM.YYYY is the default; MM.DD.YYYY is recognized when the
            // supposed month is greater than 12.
            if second > 12 && second <= 31 {
                (first, second)
            } else {
                (second, first)
            }
        };
        (year, month, day)
    };

    NaiveDate::from_ymd_opt(year as i32, month, day)
        .map(|date| date.format("%Y-%m-%d").to_string())
        .ok_or_else(|| format!("Invalid date value: {value}"))
}

fn parse_date(value: &str, format: Option<&str>) -> Result<String, String> {
    normalize_date(value, format)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_common_csv_values() {
        assert_eq!(normalize_amount("$1,234.50").unwrap(), 1234.5);
        assert!(normalize_amount("NaN").is_err());
        assert!(normalize_amount("inf").is_err());
        assert!(scaled_amount(f64::MAX, 2.0).is_err());
        assert_eq!(normalize_date("2026/9/2", None).unwrap(), "2026-09-02");
        assert_eq!(normalize_date("02.09.2026", None).unwrap(), "2026-09-02");
        assert_eq!(normalize_date("2026-31-01", None).unwrap(), "2026-01-31");
        assert_eq!(normalize_date("01.31.2026", None).unwrap(), "2026-01-31");
        assert_eq!(
            normalize_date("01.02.2026", Some("mdy")).unwrap(),
            "2026-01-02"
        );
        assert!(normalize_date("not-a-date", None).is_err());
    }
}
