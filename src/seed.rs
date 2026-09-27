use chrono::Utc;
use serde_json::{json, Value};
use sqlx::PgPool;

use crate::config::Config;
use crate::snapshots;
use crate::util::snake_case;

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env();
    let pool = sqlx::postgres::PgPoolOptions::new().max_connections(5).connect(&config.database_url).await?;
    sqlx::migrate!().run(&pool).await?;
    seed(&pool, &config).await?;
    Ok(())
}

pub async fn seed(pool: &PgPool, config: &Config) -> Result<(), sqlx::Error> {
    let fixture: Value = serde_json::from_str(&std::fs::read_to_string("fixtures/fixtures.json").expect("fixtures/fixtures.json"))
        .expect("fixtures json");
    let mut tx = pool.begin().await?;
    let now = Utc::now().naive_utc().format("%Y-%m-%d %H:%M:%S").to_string();
    let hero = fixture.get("heroImage").and_then(Value::as_str).unwrap_or("").to_string();
    let content = fixture.get("sampleRichContent").cloned().unwrap_or(Value::Array(vec![]));
    insert_named(&mut tx, &fixture, "destinationSeeds", "destinations", |row: &mut serde_json::Map<String, Value>| {
        row.insert("eyebrow".into(), Value::String("Field guide".into()));
        row.insert("content".into(), content.clone());
        row.insert("heroImage".into(), Value::String(hero.clone()));
        let name = row.get("name").and_then(Value::as_str).unwrap_or("").to_string();
        let summary = row.get("summary").and_then(Value::as_str).unwrap_or("").to_string();
        row.insert("seoTitle".into(), Value::String(format!("{name} hotels and travel guide")));
        row.insert("seoDescription".into(), Value::String(summary));
        row.insert("status".into(), Value::String("published".into()));
        row.insert("publishedAt".into(), Value::String(now.clone()));
    }).await?;
    insert_named(&mut tx, &fixture, "hotelSeeds", "hotels", |row: &mut serde_json::Map<String, Value>| {
        row.insert("publishedAt".into(), Value::String(now.clone()));
    }).await?;
    insert_named(&mut tx, &fixture, "amenitySeeds", "amenities", |_| {}).await?;
    insert_named(&mut tx, &fixture, "roomSeeds", "rooms", |_| {}).await?;
    insert_named(&mut tx, &fixture, "hotelGalleryImageSeeds", "hotel_gallery_images", |_| {}).await?;
    insert_named(&mut tx, &fixture, "hotelHighlightSeeds", "hotel_highlights", |_| {}).await?;
    insert_named(&mut tx, &fixture, "hotelFactSeeds", "hotel_facts", |_| {}).await?;
    insert_named(&mut tx, &fixture, "hotelFaqSeeds", "hotel_faqs", |_| {}).await?;
    insert_named(&mut tx, &fixture, "hotelNearbyPlaceSeeds", "hotel_nearby_places", |_| {}).await?;
    insert_named(&mut tx, &fixture, "hotelPolicySeeds", "hotel_policies", |_| {}).await?;
    insert_named(&mut tx, &fixture, "hotelReviewScoreSeeds", "hotel_review_scores", |_| {}).await?;
    insert_named(&mut tx, &fixture, "hotelReviewSeeds", "hotel_reviews", |_| {}).await?;
    insert_named(&mut tx, &fixture, "offerSeeds", "offers", |_| {}).await?;
    insert_named(&mut tx, &fixture, "postSeeds", "blog_posts", |row: &mut serde_json::Map<String, Value>| {
        row.insert("publishedAt".into(), Value::String(now.clone()));
    }).await?;
    link_amenities(&mut tx, &fixture).await?;
    ensure_admin(&mut tx, config).await?;
    tx.commit().await?;
    snapshots::queue_missing(pool).await?;
    Ok(())
}

async fn insert_named(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    fixture: &Value,
    name: &str,
    table: &str,
    customize: impl Fn(&mut serde_json::Map<String, Value>),
) -> Result<(), sqlx::Error> {
    let Some(rows) = fixture.get(name).and_then(Value::as_array) else {
        return Ok(());
    };
    if rows.is_empty() {
        return Ok(());
    }
    for row in rows {
        let mut object = row.as_object().cloned().unwrap_or_default();
        customize(&mut object);
        let mut snake = serde_json::Map::new();
        for (key, value) in object {
            snake.insert(snake_case(&key), value);
        }
        let columns: Vec<String> = snake.keys().cloned().collect();
        if columns.is_empty() {
            continue;
        }
        let quoted = columns.iter().map(|column| format!("\"{column}\"")).collect::<Vec<_>>().join(", ");
        let sql = format!(
            "INSERT INTO \"{table}\" ({quoted}) SELECT {quoted} FROM jsonb_populate_recordset(NULL::{table}, $1::jsonb) ON CONFLICT DO NOTHING"
        );
        sqlx::query(&sql).bind(Value::Array(vec![Value::Object(snake)])).execute(&mut **tx).await?;
    }
    Ok(())
}

async fn link_amenities(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, fixture: &Value) -> Result<(), sqlx::Error> {
    let amenities = fixture.get("amenitySeeds").and_then(Value::as_array).cloned().unwrap_or_default();
    let generated_hotels = fixture.get("generatedHotelSeeds").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut hotel_links = Vec::new();
    for (hotel_index, hotel) in generated_hotels.iter().enumerate() {
        for index in 0..amenities.len().min(13) {
            if (index + hotel_index) % 3 != 0 {
                hotel_links.push(json!({
                    "hotel_id": hotel.get("id").and_then(Value::as_str).unwrap_or(""),
                    "amenity_id": amenities[index].get("id").and_then(Value::as_str).unwrap_or(""),
                }));
            }
        }
    }
    if let Some(extra) = fixture.get("bengaluruAmenitySeeds").and_then(Value::as_array) {
        for amenity in extra {
            if amenity.get("appliesTo").and_then(Value::as_str) != Some("room") {
                hotel_links.push(json!({
                    "hotel_id": "hotel-bengaluru-jayanagar",
                    "amenity_id": amenity.get("id").and_then(Value::as_str).unwrap_or(""),
                }));
            }
        }
    }
    insert_links(tx, "hotel_amenities", "hotel_id", hotel_links).await?;
    let generated_rooms = fixture.get("generatedRoomSeeds").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut room_links = Vec::new();
    for (room_index, room) in generated_rooms.iter().enumerate() {
        let mut count = 0;
        for index in 0..amenities.len().min(13) {
            if count >= 4 || (index + room_index) % 2 != 0 {
                continue;
            }
            room_links.push(json!({
                "room_id": room.get("id").and_then(Value::as_str).unwrap_or(""),
                "amenity_id": amenities[index].get("id").and_then(Value::as_str).unwrap_or(""),
            }));
            count += 1;
        }
    }
    if let Some(rooms) = fixture.get("roomSeeds").and_then(Value::as_array) {
        for room in rooms {
            if room.get("hotelId").and_then(Value::as_str) == Some("hotel-bengaluru-jayanagar") {
                for amenity_id in ["amenity-free-wifi", "amenity-air-conditioning", "amenity-ensuite-bathroom", "amenity-personal-locker"] {
                    room_links.push(json!({"room_id": room.get("id").and_then(Value::as_str).unwrap_or(""), "amenity_id": amenity_id}));
                }
            }
        }
    }
    insert_links(tx, "room_amenities", "room_id", room_links).await
}

async fn insert_links(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, table: &str, id_column: &str, rows: Vec<Value>) -> Result<(), sqlx::Error> {
    if rows.is_empty() {
        return Ok(());
    }
    let sql = format!(
        "INSERT INTO \"{table}\" (\"{id_column}\", amenity_id) SELECT \"{id_column}\", amenity_id FROM jsonb_populate_recordset(NULL::{table}, $1::jsonb) ON CONFLICT DO NOTHING"
    );
    sqlx::query(&sql).bind(Value::Array(rows)).execute(&mut **tx).await?;
    Ok(())
}

async fn ensure_admin(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, config: &Config) -> Result<(), sqlx::Error> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM admin_users").fetch_one(&mut **tx).await?;
    if count > 0 {
        return Ok(());
    }
    let development = config.app_env.eq_ignore_ascii_case("development");
    let email = config.admin_email.clone().or_else(|| development.then(|| "admin@example.com".into()));
    let password = config.admin_password.clone().or_else(|| development.then(|| "ChangeMe123!".into()));
    let (Some(email), Some(password)) = (email, password) else {
        panic!("Set ADMIN_EMAIL and ADMIN_PASSWORD before seeding a non-development database.");
    };
    let hash = bcrypt::hash(password, 12).expect("password hash");
    sqlx::query("INSERT INTO admin_users (id, email, password_hash, name) VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING")
        .bind("admin-primary")
        .bind(email)
        .bind(hash)
        .bind("Site administrator")
        .execute(&mut **tx)
        .await?;
    Ok(())
}
