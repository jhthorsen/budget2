use crate::{ImportRule, Pool, User};
use chrono::NaiveDate;
use csv::{ReaderBuilder, StringRecord};
use encoding_rs::SHIFT_JIS;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, io::Cursor, path::Path};

const ACCOUNT_HEADING: &str = "FROM_HEADING_ROW";

#[derive(Debug, Deserialize)]
pub struct ColumnMapping {
    #[serde(default)]
    pub csrf_token: String,
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
    pub skipped: usize,
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
    let mut reader = csv_reader(path, delimiter)?;
    reader
        .headers()
        .map(|headers| headers.iter().map(str::to_owned).collect())
        .map_err(|err| err.to_string())
}

pub fn suggest_columns(path: &Path) -> Result<ColumnSuggestions, String> {
    let delimiter = detect_delimiter(path)?;
    let mut reader = csv_reader(path, delimiter)?;
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
    let (date_column, description_column) = samples
        .iter()
        .find_map(|record| {
            record.iter().enumerate().find_map(|(date_index, value)| {
                parse_date(value, None).ok()?;
                let description_index = record
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| *index != date_index)
                    .max_by_key(|(_, value)| value.chars().count())?
                    .0;
                Some((
                    headers[date_index].clone(),
                    headers[description_index].clone(),
                ))
            })
        })
        .unwrap_or_default();
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

    let account_column = if samples.iter().any(|record| {
        record
            .get(
                headers
                    .iter()
                    .position(|header| header == &description_column)
                    .unwrap_or(0),
            )
            .is_some_and(is_account_heading)
    }) {
        ACCOUNT_HEADING.to_string()
    } else {
        headers
            .iter()
            .find(|header| {
                let header = header.to_lowercase();
                (header.contains("account") || header == "konto")
                    && !header.contains("inn ")
                    && !header.contains("ut ")
            })
            .cloned()
            .unwrap_or_default()
    };

    Ok(ColumnSuggestions {
        category_column: find(&["category", "kategori"]),
        expense_column: find(&["expense", "debit", "kostnad", "withdrawal", "ut"]),
        income_column: find(&["income", "credit", "deposit", "inn"]),
        account_column,
        date_column,
        date_formats,
        description_column,
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

    let mut reader = csv_reader(path, detect_delimiter(path)?)?;
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
        skipped: 0,
        errors: Vec::new(),
    };
    let mut account_heading = None;
    for (index, record) in reader.records().enumerate() {
        let row_number = index + 2;
        let record = match record {
            Ok(record) => record,
            Err(err) => {
                result.total_rows += 1;
                result.errors.push(ImportError {
                    row_number,
                    row_data: "Unable to read row".to_string(),
                    error: err.to_string(),
                });
                continue;
            }
        };

        if mapping.account_column.as_deref() == Some(ACCOUNT_HEADING) {
            if let Some(name) = mapping
                .value(&record, &mapping.description_column)
                .ok()
                .and_then(account_heading_name)
            {
                account_heading = Some(name.to_string());
                continue;
            }
        }
        result.total_rows += 1;

        match import_row(
            pool,
            user,
            household_id,
            &record,
            &mapping,
            account_heading.as_deref(),
            &rules,
            multiplier,
        )
        .await
        {
            Ok(ImportRow::Imported) => result.successful += 1,
            Ok(ImportRow::Skipped) => result.skipped += 1,
            Ok(ImportRow::NoAmount) => result.errors.push(ImportError {
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

#[derive(Debug, PartialEq, Eq)]
enum ImportRow {
    Imported,
    Skipped,
    NoAmount,
}

async fn import_row(
    pool: &Pool,
    user: &User,
    household_id: i64,
    record: &StringRecord,
    mapping: &ColumnMapping,
    account_heading: Option<&str>,
    rules: &[ImportRule],
    multiplier: f64,
) -> Result<ImportRow, String> {
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

    let account_name = account_heading
        .or_else(|| mapping.optional_value(record, mapping.account_column.as_deref()))
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

    let mut result = ImportRow::NoAmount;
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
        let amount = scaled_amount(original_amount, multiplier)?;
        let inserted = sqlx::query(
            "insert into transactions (user_id, imported_by_user_id, account_id, category_id, type, amount, original_amount, description, source, processed_at)
             select ?, ?, ?, ?, ?, ?, ?, ?, 'csv', ?
             where not exists (select 1 from transactions where source = 'csv' and account_id = ? and processed_at = ? and type = ? and amount = ? and description = ?)",
        )
        .bind(user.id)
        .bind(user.id)
        .bind(account_id)
        .bind(category_id)
        .bind(transaction_type)
        .bind(amount)
        .bind(original_amount)
        .bind(description)
        .bind(&date)
        .bind(account_id)
        .bind(&date)
        .bind(transaction_type)
        .bind(amount)
        .bind(description)
        .execute(pool)
        .await
        .map_err(|err| err.to_string())?
        .rows_affected()
            > 0;
        result = if inserted {
            ImportRow::Imported
        } else if result == ImportRow::NoAmount {
            ImportRow::Skipped
        } else {
            result
        };
    }
    Ok(result)
}

fn is_account_heading(value: &str) -> bool {
    account_heading_name(value).is_some()
}

fn account_heading_name(value: &str) -> Option<&str> {
    value
        .trim()
        .strip_prefix('【')?
        .strip_suffix('】')
        .map(str::trim)
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
    let text = csv_text(fs::read(path).map_err(|err| err.to_string())?)?;
    let mut best = (b',', 0);
    for delimiter in [b',', b';', b'\t', b'|'] {
        let columns = ReaderBuilder::new()
            .delimiter(delimiter)
            .has_headers(false)
            .flexible(true)
            .from_reader(Cursor::new(&text))
            .records()
            .filter_map(Result::ok)
            .map(|record| record.len())
            .max()
            .unwrap_or_default();
        if columns > best.1 {
            best = (delimiter, columns);
        }
    }
    Ok(best.0)
}

fn csv_reader(path: &Path, delimiter: u8) -> Result<csv::Reader<Cursor<String>>, String> {
    let text = csv_text(fs::read(path).map_err(|err| err.to_string())?)?;
    Ok(ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(true)
        .flexible(true)
        .from_reader(Cursor::new(table_csv(text, delimiter)?)))
}

fn table_csv(text: String, delimiter: u8) -> Result<String, String> {
    let mut source = ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(false)
        .flexible(true)
        .from_reader(Cursor::new(text));
    let mut records = Vec::new();
    let mut first_widest = 0;
    let mut widest = 0;
    for record in source.records() {
        let record = record.map_err(|err| err.to_string())?;
        if record.len() > widest {
            widest = record.len();
            first_widest = records.len();
        }
        records.push(record);
    }
    let mut csv = csv::WriterBuilder::new()
        .delimiter(delimiter)
        .from_writer(Vec::new());
    for record in records
        .into_iter()
        .skip(first_widest)
        .filter(|record| record.len() == widest)
    {
        csv.write_record(record.iter())
            .map_err(|err| err.to_string())?;
    }
    String::from_utf8(csv.into_inner().map_err(|err| err.to_string())?)
        .map_err(|err| err.to_string())
}

fn csv_text(bytes: Vec<u8>) -> Result<String, String> {
    let text = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(error) => {
            let bytes = error.into_bytes();
            let (text, _, had_errors) = SHIFT_JIS.decode(&bytes);
            if had_errors {
                return Err("CSV must be valid UTF-8 or Shift_JIS".to_string());
            }
            text.into_owned()
        }
    }
    .replace("\r\n", "\n")
    .replace('\r', "\n");
    Ok(text)
}

fn normalize_date(value: &str, format: Option<&str>) -> Result<String, String> {
    let value = value.trim().replace(['年', '月'], "-").replace('日', "");
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

    #[test]
    fn decodes_shift_jis_csv() {
        let (encoded, _, _) = SHIFT_JIS.encode("利用日,利用内容\r2024/11/1,東京ガス\r");
        assert_eq!(
            csv_text(encoded.into_owned()).unwrap(),
            "利用日,利用内容\n2024/11/1,東京ガス\n"
        );
    }

    #[test]
    fn skips_csv_preamble_before_widest_table() {
        let table = table_csv(
            "Card statement\nCard,Number\nGold,1234\nDate,Description,Amount\n2024/11/1,Coffee,500\nFooter\n"
                .into(),
            b',',
        )
        .unwrap();
        assert!(table.starts_with("Date,Description,Amount\n2024/11/1,Coffee,500\n"));
        assert!(!table.contains("Footer"));
    }

    #[tokio::test]
    async fn skips_duplicate_imported_transactions() {
        let pool = crate::build_pool("sqlite::memory:", true).await.unwrap();
        let user = User {
            id: 1,
            ..Default::default()
        };
        sqlx::query("insert into users (id, email, name, oauth_provider, oauth_id) values (1, 'user@example.com', 'User', 'test', 'user')")
            .execute(&pool).await.unwrap();
        sqlx::query("insert into households (id, name) values (1, 'Home')")
            .execute(&pool)
            .await
            .unwrap();
        let mapping = ColumnMapping {
            csrf_token: String::new(),
            file_id: String::new(),
            date_column: "Date".into(),
            date_format: None,
            income_column: String::new(),
            expense_column: "Amount".into(),
            description_column: "Description".into(),
            account_column: Some("Account".into()),
            account_fixed_name: None,
            category_column: None,
            currency_multiplier: None,
            header_map: [
                ("Date".into(), 0),
                ("Description".into(), 1),
                ("Amount".into(), 2),
                ("Account".into(), 3),
            ]
            .into(),
        };
        let record = StringRecord::from(vec!["2026-01-01", "Coffee", "10", "Checking"]);

        assert_eq!(
            import_row(&pool, &user, 1, &record, &mapping, None, &[], 1.0)
                .await
                .unwrap(),
            ImportRow::Imported
        );
        assert_eq!(
            import_row(&pool, &user, 1, &record, &mapping, None, &[], 1.0)
                .await
                .unwrap(),
            ImportRow::Skipped
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("select count(*) from transactions")
                .fetch_one(&pool)
                .await
                .unwrap(),
            1
        );
    }

    #[test]
    fn recognizes_bracketed_account_headings() {
        assert_eq!(
            account_heading_name(" 【兼松　美保子　様】 "),
            Some("兼松　美保子　様")
        );
        assert_eq!(account_heading_name("Coffee shop"), None);
    }

    #[test]
    fn suggests_date_and_description_from_values() {
        let path = std::env::temp_dir().join(format!(
            "budget2-csv-suggest-values-test-{}.csv",
            std::process::id()
        ));
        fs::write(
            &path,
            "A,B,C\n2025/08/01,A longer transaction description,500\n",
        )
        .unwrap();
        let suggestions = suggest_columns(&path).unwrap();
        fs::remove_file(&path).unwrap();

        assert_eq!(suggestions.date_column, "A");
        assert_eq!(suggestions.description_column, "B");
    }

    #[test]
    fn suggests_japanese_statement_columns_and_headings() {
        let path =
            std::env::temp_dir().join(format!("budget2-csv-test-{}.csv", std::process::id()));
        fs::write(
            &path,
            "ご利用日,ご利用店名,ご利用金額（円）\n,【兼松　美保子　様】,\n2025年8月1日,Coffee,500\n",
        )
        .unwrap();
        let suggestions = suggest_columns(&path).unwrap();
        fs::remove_file(&path).unwrap();

        assert_eq!(suggestions.date_column, "ご利用日");
        assert_eq!(suggestions.description_column, "ご利用店名");
        assert_eq!(suggestions.expense_column, "ご利用金額（円）");
        assert_eq!(suggestions.account_column, ACCOUNT_HEADING);
    }
}
