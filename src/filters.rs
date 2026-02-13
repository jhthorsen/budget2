pub fn days_in_month(date: &str) -> askama::Result<i64> {
    let date = date.split("-").collect::<Vec<_>>();

    if date.len() >= 2
        && let (Ok(year), Ok(month)) = (date[0].parse::<i32>(), date[1].parse::<u32>())
    {
        Ok(chrono::NaiveDate::from_ymd_opt(year, month, 1)
            .and_then(|d| {
                if month == 12 {
                    chrono::NaiveDate::from_ymd_opt(year + 1, 1, 1)
                } else {
                    chrono::NaiveDate::from_ymd_opt(year, month + 1, 1)
                }
                .map(|next| (next - d).num_days())
            })
            .unwrap_or(30))
    } else {
        Ok(30)
    }
}

pub fn extract_month(value: &str) -> askama::Result<String> {
    let parts = value.split("-").collect::<Vec<_>>();
    if parts.len() == 3 {
        Ok(format!(
            "{}-{}",
            *parts.first().unwrap_or(&"0000"),
            *parts.get(1).unwrap_or(&"01")
        ))
    } else {
        Ok(value.to_string())
    }
}

pub fn format_amount(value: &f64) -> askama::Result<String> {
    // Handle negative values
    let is_negative = *value < 0.0;
    let abs_value = value.abs();

    // Split into integer and decimal parts
    let formatted = format!("{:.2}", abs_value);
    let parts: Vec<&str> = formatted.split('.').collect();
    let integer_part = parts[0];
    let decimal_part = parts.get(1).unwrap_or(&"00");

    // Add thousand separators (dots)
    let chars: Vec<char> = integer_part.chars().collect();
    let len = chars.len();
    let mut result = String::new();

    for (i, ch) in chars.iter().enumerate() {
        result.push(*ch);
        let remaining = len - i - 1;
        if remaining > 0 && remaining % 3 == 0 {
            result.push(',');
        }
    }

    // Format with comma as decimal separator (European style)
    let final_result = format!("{}.{}", result, decimal_part);

    if is_negative {
        Ok(format!("-{}", final_result))
    } else {
        Ok(final_result)
    }
}
