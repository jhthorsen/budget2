pub const PALETTE: [&str; 11] = [
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
];

#[askama::filter_fn]
pub fn fmtdate(date: &str, _env: &dyn askama::Values, format: &str) -> askama::Result<String> {
    let date = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|err| askama::Error::Custom(Box::new(err)))?;
    Ok(date.format(format).to_string())
}

#[askama::filter_fn]
pub fn extract_month(value: &str, _env: &dyn askama::Values) -> askama::Result<String> {
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

#[askama::filter_fn]
pub fn format_amount(value: &f64, _env: &dyn askama::Values) -> askama::Result<String> {
    // Handle negative values
    let is_negative = *value < 0.0;
    let abs_value = value.abs();

    // Split into integer and decimal parts
    let formatted = format!("{:.2}", abs_value);
    let parts: Vec<&str> = formatted.split('.').collect();
    let integer_part = parts[0];

    // Add thousand separators (dots)
    let chars: Vec<char> = integer_part.chars().collect();
    let len = chars.len();
    let mut result = String::new();

    for (i, ch) in chars.iter().enumerate() {
        result.push(*ch);
        let remaining = len - i - 1;
        if remaining > 0 && remaining.is_multiple_of(3) {
            result.push(',');
        }
    }

    if is_negative {
        Ok(format!("-{result}"))
    } else {
        Ok(result)
    }
}

#[askama::filter_fn]
pub fn sorted_hashmap<T: Clone>(
    map: &std::collections::HashMap<String, T>,
    _env: &dyn askama::Values,
) -> askama::Result<Vec<(usize, String, T)>> {
    let mut keys: Vec<String> = map.keys().cloned().collect();
    keys.sort();

    Ok(keys
        .into_iter()
        .enumerate()
        .map(|(i, key)| (i, key.clone(), map[&key].clone()))
        .collect())
}
