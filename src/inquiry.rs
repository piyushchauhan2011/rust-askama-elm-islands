use std::collections::HashMap;

use chrono::NaiveDate;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

pub struct InquiryResult {
    pub id: Option<String>,
    pub errors: HashMap<String, Vec<String>>,
}

pub async fn submit(pool: &PgPool, input: &Value) -> Result<InquiryResult, sqlx::Error> {
    let mut errors = HashMap::new();
    if !input.is_object() {
        errors.insert("body".into(), vec!["Valid JSON object is required".into()]);
        return Ok(InquiryResult { id: None, errors });
    }
    let hotel_id = field(input, &mut errors, "hotelId", 1, 160, true);
    let room_id = field(input, &mut errors, "roomId", 0, 160, false);
    let offer_id = field(input, &mut errors, "offerId", 0, 160, false);
    let check_in = date_field(input, &mut errors, "checkIn");
    let check_out = date_field(input, &mut errors, "checkOut");
    let adults = party(input, &mut errors, "adults", 1, 12, None);
    let children = party(input, &mut errors, "children", 0, 8, Some(0));
    let name = field(input, &mut errors, "name", 2, 100, true);
    let email = field(input, &mut errors, "email", 1, 160, true);
    let phone = field(input, &mut errors, "phone", 0, 40, false);
    let message = field(input, &mut errors, "message", 0, 1200, false);
    let website = field(input, &mut errors, "website", 0, 0, false);
    if website.is_some() {
        errors.insert("website".into(), vec!["Must be empty".into()]);
    }
    if let Some(email) = &email {
        if !valid_email(email) {
            errors.insert("email".into(), vec!["Enter a valid email address".into()]);
        }
    }
    if let (Some(check_in), Some(check_out)) = (&check_in, &check_out) {
        if check_out.as_str() <= check_in.as_str() {
            errors.insert("checkOut".into(), vec!["Check-out must be after check-in".into()]);
        }
    }
    if !errors.is_empty() {
        return Ok(InquiryResult { id: None, errors });
    }
    let hotel_id = hotel_id.unwrap();
    let visible = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM hotels h JOIN destinations d ON d.id = h.destination_id
         WHERE h.id = $1 AND h.status = 'published' AND d.status = 'published'",
    )
    .bind(&hotel_id)
    .fetch_one(pool)
    .await?;
    if visible == 0 {
        errors.insert("hotelId".into(), vec!["Hotel is not available for inquiries".into()]);
    }
    if let Some(room_id) = &room_id {
        let found = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM rooms WHERE id = $1 AND hotel_id = $2 AND status = 'published'",
        )
        .bind(room_id)
        .bind(&hotel_id)
        .fetch_one(pool)
        .await?;
        if found == 0 {
            errors.insert("roomId".into(), vec!["Room is not available at this hotel".into()]);
        }
    }
    if let Some(offer_id) = &offer_id {
        let found = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM offers WHERE id = $1 AND hotel_id = $2 AND status = 'published'",
        )
        .bind(offer_id)
        .bind(&hotel_id)
        .fetch_one(pool)
        .await?;
        if found == 0 {
            errors.insert("offerId".into(), vec!["Offer is not available at this hotel".into()]);
        }
    }
    if !errors.is_empty() {
        return Ok(InquiryResult { id: None, errors });
    }
    let id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO inquiries (id, hotel_id, room_id, offer_id, check_in, check_out, adults, children, name, email, phone, message, status)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,'new')",
    )
    .bind(&id)
    .bind(&hotel_id)
    .bind(&room_id)
    .bind(&offer_id)
    .bind(check_in.unwrap())
    .bind(check_out.unwrap())
    .bind(adults)
    .bind(children)
    .bind(name.unwrap())
    .bind(email.unwrap())
    .bind(&phone)
    .bind(&message)
    .execute(pool)
    .await?;
    Ok(InquiryResult { id: Some(id), errors })
}

pub fn success_body(id: &str) -> Value {
    json!({
        "id": id,
        "reference": id.chars().take(8).collect::<String>().to_uppercase(),
        "message": "Inquiry received. This is not a reservation; no payment has been taken."
    })
}

fn field(input: &Value, errors: &mut HashMap<String, Vec<String>>, key: &str, min: usize, max: usize, required: bool) -> Option<String> {
    match input.get(key) {
        None | Some(Value::Null) => {
            if required {
                errors.insert(key.into(), vec!["Required".into()]);
            }
            None
        }
        Some(Value::String(value)) => {
            let value = value.trim();
            if value.is_empty() && !required {
                return None;
            }
            if value.len() < min || value.len() > max {
                errors.insert(key.into(), vec![format!("Must be between {min} and {max} characters")]);
            }
            Some(value.to_string())
        }
        Some(_) => {
            errors.insert(key.into(), vec!["Must be a string".into()]);
            None
        }
    }
}

fn party(input: &Value, errors: &mut HashMap<String, Vec<String>>, key: &str, min: i32, max: i32, default_value: Option<i32>) -> i32 {
    match input.get(key) {
        None | Some(Value::Null) => {
            if let Some(default_value) = default_value {
                default_value
            } else {
                errors.insert(key.into(), vec!["Required".into()]);
                0
            }
        }
        Some(Value::Number(number)) => match number.as_i64() {
            Some(value) if (min as i64..=max as i64).contains(&value) => value as i32,
            _ => {
                errors.insert(key.into(), vec![format!("Must be an integer between {min} and {max}")]);
                0
            }
        },
        Some(Value::String(value)) => match value.parse::<i32>() {
            Ok(value) if (min..=max).contains(&value) => value,
            _ => {
                errors.insert(key.into(), vec![format!("Must be an integer between {min} and {max}")]);
                0
            }
        },
        Some(_) => {
            errors.insert(key.into(), vec![format!("Must be an integer between {min} and {max}")]);
            0
        }
    }
}

fn date_field(input: &Value, errors: &mut HashMap<String, Vec<String>>, key: &str) -> Option<String> {
    let date = field(input, errors, key, 10, 10, true)?;
    if NaiveDate::parse_from_str(&date, "%Y-%m-%d").is_err() {
        errors.insert(key.into(), vec!["Must be a valid YYYY-MM-DD date".into()]);
    }
    Some(date)
}

fn valid_email(email: &str) -> bool {
    let Some((local, host)) = email.split_once('@') else {
        return false;
    };
    !local.is_empty() && host.contains('.') && !host.starts_with('.') && !host.ends_with('.') && !email.contains(' ')
}
