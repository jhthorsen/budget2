#[macro_export]
macro_rules! set_if_empty {
    ($obj:expr, $field:ident, $value:expr) => {{
        if $obj.$field.is_empty() && !$value.is_empty() {
            $obj.$field = $value.into();
        }
    }};
}

#[macro_export]
macro_rules! set_if_none {
    ($obj:expr, $field:ident, $value:expr) => {{
        if $obj.$field.is_none() && $value.is_some() {
            $obj.$field = $value.clone();
        }
    }};
}

#[macro_export]
macro_rules! set_if_non_zero {
    ($obj:expr, $field:ident, $value:expr) => {{
        if $value as i64 != 0 {
            $obj.$field = $value;
        }
    }};
}
