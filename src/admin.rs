use std::collections::HashMap;
use std::io::Cursor;

use axum::body::Bytes;
use axum::extract::{Multipart, Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use axum_extra::extract::cookie::CookieJar;
use serde_json::{json, Map, Value};
use sqlx::Row;
use uuid::Uuid;

use crate::auth;
use crate::models::AdminUser;
use crate::snapshots;
use crate::util::{camelize, json_string, snake_case};
use crate::AppState;

const KINDS: &[&str] = &["destinations", "hotels", "rooms", "offers", "posts"];

fn table_for(kind: &str) -> &str {
    match kind {
        "posts" => "blog_posts",
        other => other,
    }
}

pub async fn login(State(state): State<AppState>, headers: HeaderMap, jar: CookieJar, body: Bytes) -> Response {
    if !auth::csrf_matches(&headers, cookie_header(&headers)) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "Invalid CSRF token"}))).into_response();
    }
    let Ok(input) = serde_json::from_slice::<Value>(&body) else {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "Invalid JSON."}))).into_response();
    };
    let email = json_string(&input, "email").unwrap_or_default();
    let password = json_string(&input, "password").unwrap_or_default();
    let user = sqlx::query_as::<_, AdminUser>("SELECT id, email, password_hash, name FROM admin_users WHERE email = $1")
        .bind(&email)
        .fetch_optional(&state.pool)
        .await;
    let Ok(Some(user)) = user else {
        return (StatusCode::UNAUTHORIZED, Json(json!({"error": "Email or password is incorrect."}))).into_response();
    };
    if bcrypt::verify(&password, &user.password_hash).unwrap_or(false) {
        let jar = jar.add(auth::session_cookie(&state.config, &user.id));
        (jar, Json(json!({"ok": true}))).into_response()
    } else {
        (StatusCode::UNAUTHORIZED, Json(json!({"error": "Email or password is incorrect."}))).into_response()
    }
}

pub async fn logout(State(state): State<AppState>, jar: CookieJar) -> Response {
    let jar = jar.add(auth::clear_cookie(&state.config, "hotel-admin"));
    (jar, Json(json!({"ok": true}))).into_response()
}

pub async fn me(State(state): State<AppState>, headers: HeaderMap) -> Response {
    match current_user(&state, &headers).await {
        Ok(user) => Json(json!({"id": user.id, "email": user.email, "name": user.name})).into_response(),
        Err(response) => response,
    }
}

pub async fn csrf(State(state): State<AppState>, jar: CookieJar) -> Response {
    let (cookie, token) = auth::csrf_cookie(&state.config);
    (jar.add(cookie), Json(json!({"csrfToken": token}))).into_response()
}

pub async fn dashboard(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Err(response) = current_user(&state, &headers).await {
        return response;
    }
    let destination_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM destinations").fetch_one(&state.pool).await.unwrap_or(0);
    let hotel_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM hotels").fetch_one(&state.pool).await.unwrap_or(0);
    let room_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM rooms").fetch_one(&state.pool).await.unwrap_or(0);
    let offer_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM offers").fetch_one(&state.pool).await.unwrap_or(0);
    let inquiry_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM inquiries").fetch_one(&state.pool).await.unwrap_or(0);
    let post_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM blog_posts").fetch_one(&state.pool).await.unwrap_or(0);
    let recent = sqlx::query(
        "SELECT i.id, i.name, i.email, i.status, i.check_in, i.check_out, i.created_at, h.name AS hotel_name
         FROM inquiries i JOIN hotels h ON h.id = i.hotel_id
         ORDER BY i.created_at DESC LIMIT 8",
    )
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();
    let recent_inquiries: Vec<Value> = recent
        .into_iter()
        .map(|row| {
            json!({
                "id": row.get::<String, _>("id"),
                "name": row.get::<String, _>("name"),
                "email": row.get::<String, _>("email"),
                "status": row.get::<String, _>("status"),
                "checkIn": row.get::<String, _>("check_in"),
                "checkOut": row.get::<String, _>("check_out"),
                "hotelName": row.get::<String, _>("hotel_name"),
                "createdAt": row.get::<chrono::NaiveDateTime, _>("created_at").to_string(),
            })
        })
        .collect();
    let destinations = camel_rows(&state, "SELECT id, name, slug FROM destinations ORDER BY name").await;
    let hotels = camel_rows(&state, "SELECT id, name, slug, destination_id FROM hotels ORDER BY name").await;
    let media = media_rows(&state).await;
    Json(json!({
        "destinationCount": destination_count,
        "hotelCount": hotel_count,
        "roomCount": room_count,
        "offerCount": offer_count,
        "inquiryCount": inquiry_count,
        "postCount": post_count,
        "recentInquiries": recent_inquiries,
        "destinations": destinations,
        "hotels": hotels,
        "media": media,
    }))
    .into_response()
}

pub async fn list(State(state): State<AppState>, headers: HeaderMap, Path(kind): Path<String>) -> Response {
    if let Err(response) = current_user(&state, &headers).await {
        return response;
    }
    if !KINDS.contains(&kind.as_str()) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let sql = match kind.as_str() {
        "rooms" => "SELECT r.id, r.name AS label, r.slug, r.status, h.name AS hotel_name FROM rooms r JOIN hotels h ON h.id = r.hotel_id ORDER BY h.name, r.name".to_string(),
        "offers" => "SELECT o.id, o.title AS label, o.slug, o.status, h.name AS hotel_name FROM offers o JOIN hotels h ON h.id = o.hotel_id ORDER BY h.name, o.title".to_string(),
        "posts" => "SELECT id, title AS label, slug, status FROM blog_posts ORDER BY title".to_string(),
        other => format!("SELECT id, name AS label, slug, status FROM {} ORDER BY name", table_for(other)),
    };
    Json(camel_rows_sql(&state, &sql).await).into_response()
}

pub async fn item(State(state): State<AppState>, headers: HeaderMap, Path((kind, id)): Path<(String, String)>) -> Response {
    if let Err(response) = current_user(&state, &headers).await {
        return response;
    }
    if !KINDS.contains(&kind.as_str()) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let pickers = picker_payload(&state).await;
    let media = media_rows(&state).await;
    if id == "new" {
        return Json(json!({"item": default_item(&kind), "destinations": pickers.0, "hotels": pickers.1, "media": media})).into_response();
    }
    let row = sqlx::query(&format!("SELECT to_jsonb(t) AS data FROM {} t WHERE id = $1", table_for(&kind)))
        .bind(&id)
        .fetch_optional(&state.pool)
        .await;
    match row {
        Ok(Some(row)) => {
            let data: Value = row.get("data");
            Json(json!({"item": camelize(data), "destinations": pickers.0, "hotels": pickers.1, "media": media})).into_response()
        }
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

pub async fn save(State(state): State<AppState>, headers: HeaderMap, Path(kind): Path<String>, body: Bytes) -> Response {
    if let Err(response) = require_write(&state, &headers).await {
        return response;
    }
    if !KINDS.contains(&kind.as_str()) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let Ok(input) = serde_json::from_slice::<Value>(&body) else {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "Invalid JSON."}))).into_response();
    };
    if !input.is_object() {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "Content must be an object."}))).into_response();
    }
    match save_record(&state, &kind, &input).await {
        Ok(id) => Json(json!({"id": id})).into_response(),
        Err(SaveError::Validation(errors)) => (StatusCode::BAD_REQUEST, Json(json!({"errors": errors}))).into_response(),
        Err(SaveError::Conflict) => (StatusCode::CONFLICT, Json(json!({"error": "A record with this slug already exists in its catalog scope.", "field": "slug"}))).into_response(),
        Err(SaveError::NotFound) => StatusCode::NOT_FOUND.into_response(),
        Err(SaveError::Database(error)) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

pub async fn publish(State(state): State<AppState>, headers: HeaderMap, Path((kind, id)): Path<(String, String)>, body: Bytes) -> Response {
    if let Err(response) = require_write(&state, &headers).await {
        return response;
    }
    if !KINDS.contains(&kind.as_str()) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let requested = serde_json::from_slice::<Value>(&body).ok().and_then(|value| json_string(&value, "status"));
    match toggle_publish(&state, &kind, &id, requested.as_deref()).await {
        Ok(status) => Json(json!({"id": id, "status": status})).into_response(),
        Err(SaveError::NotFound) => StatusCode::NOT_FOUND.into_response(),
        Err(SaveError::Database(error)) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
        Err(_) => StatusCode::BAD_REQUEST.into_response(),
    }
}

pub async fn inquiry_status(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<String>, body: Bytes) -> Response {
    if let Err(response) = require_write(&state, &headers).await {
        return response;
    }
    let Ok(input) = serde_json::from_slice::<Value>(&body) else {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "Invalid JSON."}))).into_response();
    };
    let status = json_string(&input, "status").unwrap_or_default();
    if !matches!(status.as_str(), "new" | "contacted" | "closed") {
        return (StatusCode::BAD_REQUEST, Json(json!({"errors": {"status": ["Must be new, contacted, or closed"]}}))).into_response();
    }
    let result = sqlx::query("UPDATE inquiries SET status = $1, updated_at = now() WHERE id = $2")
        .bind(&status)
        .bind(&id)
        .execute(&state.pool)
        .await;
    match result {
        Ok(done) if done.rows_affected() == 1 => Json(json!({"id": id, "status": status})).into_response(),
        Ok(_) => StatusCode::NOT_FOUND.into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

pub async fn inquiries(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Err(response) = current_user(&state, &headers).await {
        return response;
    }
    let rows = sqlx::query(
        "SELECT i.id, i.hotel_id, i.room_id, i.offer_id, i.check_in, i.check_out, i.adults, i.children,
                i.name, i.email, i.phone, i.message, i.status, i.created_at, h.name AS hotel_name
         FROM inquiries i JOIN hotels h ON h.id = i.hotel_id
         ORDER BY i.created_at DESC",
    )
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();
    let items: Vec<Value> = rows
        .into_iter()
        .map(|row| {
            json!({
                "id": row.get::<String, _>("id"),
                "hotelId": row.get::<String, _>("hotel_id"),
                "hotelName": row.get::<String, _>("hotel_name"),
                "checkIn": row.get::<String, _>("check_in"),
                "checkOut": row.get::<String, _>("check_out"),
                "adults": row.get::<i32, _>("adults"),
                "children": row.get::<i32, _>("children"),
                "name": row.get::<String, _>("name"),
                "email": row.get::<String, _>("email"),
                "phone": row.get::<Option<String>, _>("phone"),
                "message": row.get::<Option<String>, _>("message"),
                "status": row.get::<String, _>("status"),
                "createdAt": row.get::<chrono::NaiveDateTime, _>("created_at").to_string(),
            })
        })
        .collect();
    Json(items).into_response()
}

pub async fn upload_media(State(state): State<AppState>, headers: HeaderMap, mut multipart: Multipart) -> Response {
    if let Err(response) = require_write(&state, &headers).await {
        return response;
    }
    let mut bytes = None;
    let mut filename = String::from("upload");
    let mut alt = String::new();
    let mut caption = String::new();
    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        if name == "file" {
            filename = field.file_name().unwrap_or("upload").to_string();
            match field.bytes().await {
                Ok(data) if data.len() <= 10 * 1024 * 1024 => bytes = Some(data),
                Ok(_) => return (StatusCode::BAD_REQUEST, Json(json!({"error": "Images must be no larger than 10 MB."}))).into_response(),
                Err(_) => return (StatusCode::BAD_REQUEST, Json(json!({"error": "Could not read the upload."}))).into_response(),
            }
        } else if name == "alt" {
            alt = field.text().await.unwrap_or_default();
        } else if name == "caption" {
            caption = field.text().await.unwrap_or_default();
        }
    }
    let Some(bytes) = bytes else {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "Choose an image file."}))).into_response();
    };
    let image = match image::load_from_memory(&bytes) {
        Ok(image) => image,
        Err(_) => return (StatusCode::BAD_REQUEST, Json(json!({"error": "Upload must be a JPEG, PNG, GIF, or WebP image."}))).into_response(),
    };
    let id = Uuid::new_v4().to_string();
    let widths = [("thumbnail", 160u32), ("small", 480), ("medium", 960), ("large", 1600), ("original", 2400)];
    let mut variants = Map::new();
    let directory = state.config.media_dir.join(&id);
    if std::fs::create_dir_all(&directory).is_err() {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Could not store media."}))).into_response();
    }
    for (name, width) in widths {
        let resized = if image.width() > width {
            image.resize(width, u32::MAX, image::imageops::FilterType::Lanczos3)
        } else {
            image.clone()
        };
        let mut encoded = Cursor::new(Vec::new());
        if resized.write_to(&mut encoded, image::ImageFormat::WebP).is_err() {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Could not encode image."}))).into_response();
        }
        if std::fs::write(directory.join(format!("{name}.webp")), encoded.get_ref()).is_err() {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Could not store media."}))).into_response();
        }
        variants.insert(name.into(), Value::String(format!("/media/{id}/{name}")));
    }
    let alt = alt.trim().to_string();
    if alt.len() < 2 {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "Alternative text is required."}))).into_response();
    }
    let caption = {
        let trimmed = caption.trim().to_string();
        if trimmed.is_empty() { None } else { Some(trimmed) }
    };
    let variants = Value::Object(variants);
    let result = sqlx::query(
        "INSERT INTO media_assets (id, filename, mime_type, width, height, alt, caption, focal_x, focal_y, variants)
         VALUES ($1,$2,'image/webp',$3,$4,$5,$6,0.5,0.5,$7)",
    )
    .bind(&id)
    .bind(&filename)
    .bind(image.width() as i32)
    .bind(image.height() as i32)
    .bind(&alt)
    .bind(&caption)
    .bind(&variants)
    .execute(&state.pool)
    .await;
    match result {
        Ok(_) => Json(json!({"id": id, "alt": alt, "variants": variants})).into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

pub async fn media(State(state): State<AppState>, Path((id, variant)): Path<(String, String)>) -> Response {
    if !matches!(variant.as_str(), "thumbnail" | "small" | "medium" | "large" | "original") || id.contains('/') || id.contains('.') {
        return StatusCode::NOT_FOUND.into_response();
    }
    let path = state.config.media_dir.join(&id).join(format!("{variant}.webp"));
    match tokio::fs::read(path).await {
        Ok(bytes) => {
            ([(header::CONTENT_TYPE, "image/webp"), (header::CACHE_CONTROL, "public, max-age=31536000, immutable")], bytes).into_response()
        }
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

enum SaveError {
    Validation(HashMap<String, Vec<String>>),
    Conflict,
    NotFound,
    Database(sqlx::Error),
}

impl From<sqlx::Error> for SaveError {
    fn from(error: sqlx::Error) -> Self {
        if let sqlx::Error::Database(db) = &error {
            if db.code().as_deref() == Some("23505") {
                return SaveError::Conflict;
            }
        }
        SaveError::Database(error)
    }
}

async fn save_record(state: &AppState, kind: &str, input: &Value) -> Result<String, SaveError> {
    let mut values = normalized(kind, input)?;
    let existing = json_string(input, "id").filter(|id| id != "new");
    let mut tx = state.pool.begin().await?;
    let (old_path, old_destination, old_hotel) = if let Some(id) = &existing {
        previous_path(&mut tx, kind, id).await?
    } else {
        (None, None, None)
    };
    let id = if let Some(id) = existing {
        update_row(&mut tx, kind, &id, &values).await?;
        id
    } else {
        let id = Uuid::new_v4().to_string();
        values.insert("id".into(), Value::String(id.clone()));
        insert_row(&mut tx, kind, &values).await?;
        id
    };
    if let Some(Value::String(status)) = values.get("status").cloned() {
        if matches!(kind, "destinations" | "hotels" | "posts") {
            let sql = if status == "published" {
                format!("UPDATE {} SET published_at = COALESCE(published_at, now()) WHERE id = $1", table_for(kind))
            } else {
                format!("UPDATE {} SET published_at = NULL WHERE id = $1", table_for(kind))
            };
            sqlx::query(&sql).bind(&id).execute(&mut *tx).await?;
        }
    }
    snapshots::invalidate(&mut tx, kind, &id, old_path.as_deref(), old_destination.as_deref(), old_hotel.as_deref()).await?;
    tx.commit().await?;
    Ok(id)
}

async fn toggle_publish(state: &AppState, kind: &str, id: &str, requested: Option<&str>) -> Result<String, SaveError> {
    let mut tx = state.pool.begin().await?;
    let table = table_for(kind);
    let current: Option<String> = sqlx::query_scalar(&format!("SELECT status FROM {table} WHERE id = $1"))
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
    let Some(current) = current else {
        return Err(SaveError::NotFound);
    };
    let (old_path, old_destination, old_hotel) = previous_path(&mut tx, kind, id).await?;
    let next = match requested {
        Some("published") | Some("draft") => requested.unwrap().to_string(),
        _ => if current == "published" { "draft" } else { "published" }.to_string(),
    };
    let published_at = if matches!(kind, "destinations" | "hotels" | "posts") {
        if next == "published" { "now()" } else { "NULL" }
    } else {
        ""
    };
    let sql = if published_at.is_empty() {
        format!("UPDATE {table} SET status = $1, updated_at = now() WHERE id = $2")
    } else if next == "published" {
        format!("UPDATE {table} SET status = $1, published_at = now(), updated_at = now() WHERE id = $2")
    } else {
        format!("UPDATE {table} SET status = $1, published_at = NULL, updated_at = now() WHERE id = $2")
    };
    sqlx::query(&sql).bind(&next).bind(id).execute(&mut *tx).await?;
    snapshots::invalidate(&mut tx, kind, id, old_path.as_deref(), old_destination.as_deref(), old_hotel.as_deref()).await?;
    tx.commit().await?;
    Ok(next)
}

fn normalized(kind: &str, input: &Value) -> Result<Map<String, Value>, SaveError> {
    let mut errors = HashMap::new();
    let mut values = Map::new();
    let require = |values: &mut Map<String, Value>, errors: &mut HashMap<String, Vec<String>>, key: &str, min: usize, max: usize| {
        match json_string(input, key) {
            Some(value) if (min..=max).contains(&value.len()) => {
                values.insert(snake_case(key), Value::String(value));
            }
            Some(_) => {
                errors.insert(key.into(), vec![format!("Must be between {min} and {max} characters")]);
            }
            None => {
                errors.insert(key.into(), vec!["Required".into()]);
            }
        }
    };
    match kind {
        "destinations" => {
            require(&mut values, &mut errors, "name", 1, 120);
            require(&mut values, &mut errors, "country", 1, 80);
            require(&mut values, &mut errors, "summary", 1, 500);
            optional_text(&mut values, input, "eyebrow", "Destination");
            optional_text(&mut values, input, "heroImage", "");
            optional_text(&mut values, input, "seoTitle", "");
            optional_text(&mut values, input, "seoDescription", "");
            values.insert("content".into(), input.get("content").cloned().unwrap_or(Value::Array(vec![])));
        }
        "hotels" => {
            require(&mut values, &mut errors, "name", 1, 160);
            require(&mut values, &mut errors, "destinationId", 1, 160);
            require(&mut values, &mut errors, "summary", 1, 500);
            require(&mut values, &mut errors, "description", 1, 4000);
            require(&mut values, &mut errors, "address", 1, 240);
            optional_text(&mut values, input, "propertyType", "Boutique hotel");
            optional_text(&mut values, input, "heroImage", "");
            optional_text(&mut values, input, "currency", "USD");
            optional_text(&mut values, input, "seoTitle", "");
            optional_text(&mut values, input, "seoDescription", "");
            number_field(&mut values, &mut errors, input, "priceFrom", true);
            number_field(&mut values, &mut errors, input, "rating", true);
            number_field(&mut values, &mut errors, input, "starRating", false);
            number_field(&mut values, &mut errors, input, "reviewCount", false);
            number_field(&mut values, &mut errors, input, "latitude", false);
            number_field(&mut values, &mut errors, input, "longitude", false);
        }
        "rooms" => {
            require(&mut values, &mut errors, "name", 1, 160);
            require(&mut values, &mut errors, "hotelId", 1, 160);
            require(&mut values, &mut errors, "summary", 1, 500);
            optional_text(&mut values, input, "image", "");
            optional_text(&mut values, input, "bed", "");
            number_field(&mut values, &mut errors, input, "priceFrom", true);
            number_field(&mut values, &mut errors, input, "maxGuests", true);
            number_field(&mut values, &mut errors, input, "sizeSqm", false);
        }
        "offers" => {
            require(&mut values, &mut errors, "title", 1, 160);
            require(&mut values, &mut errors, "hotelId", 1, 160);
            require(&mut values, &mut errors, "summary", 1, 500);
            optional_text(&mut values, input, "image", "");
            optional_text(&mut values, input, "terms", "");
            optional_text(&mut values, input, "validFrom", "");
            optional_text(&mut values, input, "validTo", "");
            number_field(&mut values, &mut errors, input, "discountPercent", false);
        }
        "posts" => {
            require(&mut values, &mut errors, "title", 1, 180);
            require(&mut values, &mut errors, "excerpt", 1, 500);
            require(&mut values, &mut errors, "author", 1, 120);
            optional_text(&mut values, input, "heroImage", "");
            optional_text(&mut values, input, "seoTitle", "");
            optional_text(&mut values, input, "seoDescription", "");
            values.insert("content".into(), input.get("content").cloned().unwrap_or(Value::Array(vec![])));
        }
        _ => return Err(SaveError::NotFound),
    }
    let slug = json_string(input, "slug").unwrap_or_else(|| slugify(json_string(input, if kind == "offers" || kind == "posts" { "title" } else { "name" }).unwrap_or_default()));
    if !crate::snapshots::canonical_shape(&format!("/x/{slug}")).then_some(()).is_some() && !valid_slug(&slug) {
        errors.insert("slug".into(), vec!["Use lowercase letters, numbers, and hyphens.".into()]);
    } else if !valid_slug(&slug) {
        errors.insert("slug".into(), vec!["Use lowercase letters, numbers, and hyphens.".into()]);
    } else {
        values.insert("slug".into(), Value::String(slug));
    }
    let status = json_string(input, "status").unwrap_or_else(|| "draft".into());
    if matches!(status.as_str(), "draft" | "published") {
        values.insert("status".into(), Value::String(status));
    } else {
        errors.insert("status".into(), vec!["Must be draft or published".into()]);
    }
    if errors.is_empty() {
        Ok(values)
    } else {
        Err(SaveError::Validation(errors))
    }
}

fn valid_slug(slug: &str) -> bool {
    crate::snapshots::canonical_shape(&format!("/blog/{slug}"))
}

fn optional_text(values: &mut Map<String, Value>, input: &Value, key: &str, default: &str) {
    let value = json_string(input, key).unwrap_or_else(|| default.to_string());
    if !(value.is_empty() && matches!(key, "validFrom" | "validTo")) {
        values.insert(snake_case(key), Value::String(value));
    }
}

fn number_field(values: &mut Map<String, Value>, errors: &mut HashMap<String, Vec<String>>, input: &Value, key: &str, required: bool) {
    match input.get(key) {
        None | Some(Value::Null) if !required => {}
        None | Some(Value::Null) => {
            errors.insert(key.into(), vec!["Required".into()]);
        }
        Some(Value::Number(number)) => {
            values.insert(snake_case(key), Value::Number(number.clone()));
        }
        Some(Value::String(text)) if text.trim().is_empty() && !required => {}
        Some(Value::String(text)) => {
            if let Ok(number) = text.trim().parse::<f64>() {
                if let Some(number) = serde_json::Number::from_f64(number) {
                    values.insert(snake_case(key), Value::Number(number));
                }
            } else {
                errors.insert(key.into(), vec!["Must be a number".into()]);
            }
        }
        Some(_) => {
            errors.insert(key.into(), vec!["Must be a number".into()]);
        }
    }
}

fn slugify(value: String) -> String {
    let mut slug = String::new();
    let mut dash = false;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
            dash = false;
        } else if !dash && !slug.is_empty() {
            slug.push('-');
            dash = true;
        }
    }
    slug.trim_matches('-').to_string()
}

async fn insert_row(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, kind: &str, values: &Map<String, Value>) -> Result<(), SaveError> {
    let columns: Vec<&str> = values.keys().map(String::as_str).collect();
    let placeholders: Vec<String> = (1..=columns.len()).map(|index| format!("${index}")).collect();
    let sql = format!(
        "INSERT INTO {} ({}) VALUES ({})",
        table_for(kind),
        columns.iter().map(|column| format!("\"{column}\"")).collect::<Vec<_>>().join(", "),
        placeholders.join(", ")
    );
    let mut query = sqlx::query(&sql);
    for column in &columns {
        query = bind_value(query, &values[*column]);
    }
    query.execute(&mut **tx).await?;
    Ok(())
}

async fn update_row(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, kind: &str, id: &str, values: &Map<String, Value>) -> Result<(), SaveError> {
    let columns: Vec<&str> = values.keys().map(String::as_str).collect();
    let assignments: Vec<String> = columns.iter().enumerate().map(|(index, column)| format!("\"{column}\" = ${}", index + 1)).collect();
    let sql = format!("UPDATE {} SET {}, updated_at = now() WHERE id = ${}", table_for(kind), assignments.join(", "), columns.len() + 1);
    let mut query = sqlx::query(&sql);
    for column in &columns {
        query = bind_value(query, &values[*column]);
    }
    let done = query.bind(id).execute(&mut **tx).await?;
    if done.rows_affected() == 0 {
        Err(SaveError::NotFound)
    } else {
        Ok(())
    }
}

fn bind_value<'q>(query: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>, value: &'q Value) -> sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments> {
    match value {
        Value::String(text) => query.bind(text),
        Value::Number(number) => {
            if let Some(integer) = number.as_i64() {
                query.bind(integer)
            } else {
                query.bind(number.as_f64().unwrap_or(0.0))
            }
        }
        Value::Bool(flag) => query.bind(*flag),
        other => query.bind(other),
    }
}

async fn previous_path(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, kind: &str, id: &str) -> Result<(Option<String>, Option<String>, Option<String>), SaveError> {
    match kind {
        "destinations" => {
            let slug: Option<String> = sqlx::query_scalar("SELECT slug FROM destinations WHERE id = $1").bind(id).fetch_optional(&mut **tx).await?;
            Ok((slug.map(|slug| format!("/destinations/{slug}")), None, None))
        }
        "hotels" => {
            let row: Option<(String, String)> = sqlx::query_as("SELECT slug, destination_id FROM hotels WHERE id = $1").bind(id).fetch_optional(&mut **tx).await?;
            Ok(row.map(|(slug, destination)| (Some(format!("/hotels/{slug}")), Some(destination), None)).unwrap_or((None, None, None)))
        }
        "posts" => {
            let slug: Option<String> = sqlx::query_scalar("SELECT slug FROM blog_posts WHERE id = $1").bind(id).fetch_optional(&mut **tx).await?;
            Ok((slug.map(|slug| format!("/blog/{slug}")), None, None))
        }
        "offers" => {
            let row: Option<(String, String, String)> = sqlx::query_as(
                "SELECT o.slug, h.slug, o.hotel_id FROM offers o JOIN hotels h ON h.id = o.hotel_id WHERE o.id = $1",
            )
            .bind(id)
            .fetch_optional(&mut **tx)
            .await?;
            Ok(row.map(|(offer, hotel, hotel_id)| (Some(format!("/hotels/{hotel}/offers/{offer}")), None, Some(hotel_id))).unwrap_or((None, None, None)))
        }
        "rooms" => {
            let hotel_id: Option<String> = sqlx::query_scalar("SELECT hotel_id FROM rooms WHERE id = $1").bind(id).fetch_optional(&mut **tx).await?;
            Ok((None, None, hotel_id))
        }
        _ => Ok((None, None, None)),
    }
}

fn default_item(kind: &str) -> Value {
    json!({
        "id": "new",
        "status": "draft",
        "content": [],
        "currency": "USD",
        "propertyType": if kind == "hotels" { "Boutique hotel" } else { "" },
        "eyebrow": if kind == "destinations" { "Field guide" } else { "" }
    })
}

async fn media_rows(state: &AppState) -> Vec<Value> {
    camel_rows(state, "SELECT id, filename, alt, caption, width, height, variants FROM media_assets ORDER BY created_at DESC").await
}

async fn picker_payload(state: &AppState) -> (Vec<Value>, Vec<Value>) {
    (
        camel_rows(state, "SELECT id, name, slug FROM destinations ORDER BY name").await,
        camel_rows(state, "SELECT id, name, slug, destination_id FROM hotels ORDER BY name").await,
    )
}

async fn camel_rows(state: &AppState, sql: &str) -> Vec<Value> {
    camel_rows_sql(state, sql).await
}

async fn camel_rows_sql(state: &AppState, sql: &str) -> Vec<Value> {
    let Ok(rows) = sqlx::query(&format!("SELECT to_jsonb(t) AS data FROM ({sql}) t")).fetch_all(&state.pool).await else {
        return Vec::new();
    };
    rows.into_iter().map(|row| camelize(row.get("data"))).collect()
}

async fn current_user(state: &AppState, headers: &HeaderMap) -> Result<AdminUser, Response> {
    let Some(user_id) = headers.get(header::COOKIE).and_then(|value| value.to_str().ok()).and_then(|value| auth::read_session(&state.config, value)) else {
        return Err((StatusCode::UNAUTHORIZED, Json(json!({"error": "Sign in required."}))).into_response());
    };
    let user = sqlx::query_as::<_, AdminUser>("SELECT id, email, password_hash, name FROM admin_users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(&state.pool)
        .await
        .ok()
        .flatten();
    user.ok_or_else(|| (StatusCode::UNAUTHORIZED, Json(json!({"error": "Sign in required."}))).into_response())
}

async fn require_write(state: &AppState, headers: &HeaderMap) -> Result<AdminUser, Response> {
    let user = current_user(state, headers).await?;
    if auth::csrf_matches(headers, cookie_header(headers)) {
        Ok(user)
    } else {
        Err((StatusCode::FORBIDDEN, Json(json!({"error": "Invalid CSRF token"}))).into_response())
    }
}

fn cookie_header(headers: &HeaderMap) -> Option<&str> {
    headers.get(header::COOKIE).and_then(|value| value.to_str().ok())
}
