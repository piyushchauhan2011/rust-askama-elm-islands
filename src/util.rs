use serde_json::{Map, Value};

pub fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(character),
        }
    }
    out
}

pub fn price(amount: i32, currency: &str) -> String {
    let symbol = if currency.eq_ignore_ascii_case("USD") {
        "$"
    } else {
        ""
    };
    let negative = amount < 0;
    let digits = amount.unsigned_abs().to_string();
    let mut grouped = String::new();
    for (index, character) in digits.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(character);
    }
    let body: String = grouped.chars().rev().collect();
    if symbol.is_empty() {
        format!("{currency} {sign}{body}", sign = if negative { "-" } else { "" })
    } else {
        format!("{sign}{symbol}{body}", sign = if negative { "-" } else { "" })
    }
}

pub fn rating(value: f64) -> String {
    format!("{value:.1}")
}

pub fn snake_case(name: &str) -> String {
    let mut result = String::with_capacity(name.len() + 4);
    for character in name.chars() {
        if character.is_uppercase() && !result.is_empty() {
            result.push('_');
        }
        result.push(character.to_ascii_lowercase());
    }
    result
}

pub fn camel_key(name: &str) -> String {
    let mut result = String::new();
    let mut upper = false;
    for character in name.chars() {
        if character == '_' {
            upper = true;
        } else if upper {
            result.push(character.to_ascii_uppercase());
            upper = false;
        } else {
            result.push(character);
        }
    }
    result
}

pub fn camelize(value: Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.into_iter().map(camelize).collect()),
        Value::Object(map) => {
            let mut next = Map::new();
            for (key, item) in map {
                next.insert(camel_key(&key), camelize(item));
            }
            Value::Object(next)
        }
        other => other,
    }
}

pub fn json_string(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(|item| match item {
        Value::String(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() { None } else { Some(trimmed.to_string()) }
        }
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    })
}

pub fn constant_time_eq(left: &str, right: &str) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.bytes()
        .zip(right.bytes())
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}
