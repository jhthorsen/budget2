pub fn format_amount(value: &f64) -> ::askama::Result<String> {
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
