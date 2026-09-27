use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use elsewhere::{router, state, Config};
use http_body_util::BodyExt;
use serde_json::Value;
use tokio::sync::{Mutex, MutexGuard};
use tower::ServiceExt;

static LOCK: Mutex<()> = Mutex::const_new(());

struct Client {
    app: axum::Router,
    cookies: String,
    _guard: MutexGuard<'static, ()>,
}

impl Client {
    async fn new() -> Self {
        let _guard = LOCK.lock().await;
        let url = std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL");
        let config = Config::for_tests(url.clone());
        ensure_database(&url).await;
        let pool = elsewhere::connect(&config).await.expect("migrate");
        sqlx::query(
            "TRUNCATE TABLE inquiries, room_amenities, hotel_amenities, hotel_gallery_images, hotel_highlights, hotel_facts, hotel_faqs, hotel_nearby_places, hotel_policies, hotel_review_scores, hotel_reviews, offers, rooms, hotels, destinations, amenities, blog_posts, media_assets, page_snapshots, snapshot_jobs, admin_users CASCADE",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO destinations (id, name, slug, country, eyebrow, summary, content, hero_image, seo_title, seo_description, status)
             VALUES ('dest-amalfi', 'Amalfi Coast', 'amalfi-coast', 'Italy', 'Field guide', 'Cliffs and coves', '[]', '/images/hero-1280.webp', 'Amalfi', 'Cliffs', 'published')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO hotels (id, destination_id, name, slug, property_type, address, summary, description, hero_image, rating, review_count, price_from, currency, seo_title, seo_description, status)
             VALUES ('hotel-casa', 'dest-amalfi', 'Casa Luna', 'casa-luna', 'Boutique hotel', '1 Cliff Road', 'A quiet house', 'A longer description of the house.', '/images/hotel-pool.webp', 4.6, 2, 240, 'USD', 'Casa Luna', 'A quiet house', 'published'),
                    ('hotel-budget', 'dest-amalfi', 'Budget Nook', 'budget-nook', 'Inn', '2 Harbor', 'A simple room', 'A simple description for the inn.', '/images/hotel-garden.webp', 3.2, 1, 80, 'USD', 'Budget Nook', 'A simple room', 'published')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO amenities (id, name) VALUES ('amenity-pool', 'Pool')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO hotel_amenities (hotel_id, amenity_id) VALUES ('hotel-casa', 'amenity-pool')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO offers (id, hotel_id, title, slug, summary, image, terms, status) VALUES ('offer-stay', 'hotel-casa', 'Longer stay', 'longer-stay', 'Three nights', '/images/hotel-terrace.webp', 'Subject to availability', 'published')",
        )
        .execute(&pool)
        .await
        .unwrap();
        let hash = bcrypt::hash("ChangeMe123!", 4).unwrap();
        sqlx::query("INSERT INTO admin_users (id, email, password_hash, name) VALUES ('admin-primary', 'admin@example.com', $1, 'Site administrator')")
            .bind(hash)
            .execute(&pool)
            .await
            .unwrap();
        Self { app: router(state(pool, config)), cookies: String::new(), _guard }
    }

    async fn send(&mut self, method: &str, path: &str, body: Option<&str>, csrf: bool) -> (StatusCode, String) {
        let mut builder = Request::builder().method(method).uri(path);
        if !self.cookies.is_empty() {
            builder = builder.header(header::COOKIE, &self.cookies);
        }
        if csrf {
            if let Some(token) = cookie_value(&self.cookies, "hotel-csrf") {
                builder = builder.header("x-csrf-token", token);
            }
        }
        if body.is_some() {
            builder = builder.header(header::CONTENT_TYPE, "application/json");
        }
        let request = builder.body(Body::from(body.unwrap_or("").to_string())).unwrap();
        let response = self.app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        if let Some(set_cookie) = response.headers().get(header::SET_COOKIE) {
            remember(&mut self.cookies, set_cookie.to_str().unwrap_or(""));
        }
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (status, String::from_utf8_lossy(&bytes).into_owned())
    }
}

async fn ensure_database(url: &str) {
    let (base, name) = url.rsplit_once('/').unwrap();
    let name = name.split('?').next().unwrap();
    let admin = format!("{base}/postgres");
    let pool = sqlx::postgres::PgPoolOptions::new().connect(&admin).await.expect("postgres admin connection");
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_database WHERE datname = $1)")
        .bind(name)
        .fetch_one(&pool)
        .await
        .unwrap();
    if !exists {
        sqlx::query(&format!("CREATE DATABASE \"{name}\"")).execute(&pool).await.unwrap();
    }
}

fn remember(jar: &mut String, set_cookie: &str) {
    let pair = set_cookie.split(';').next().unwrap_or("");
    let Some((name, _)) = pair.split_once('=') else {
        return;
    };
    let mut kept = Vec::new();
    for part in jar.split(';').filter(|part| !part.trim().is_empty()) {
        if !part.trim().starts_with(&format!("{name}=")) {
            kept.push(part.trim().to_string());
        }
    }
    kept.push(pair.to_string());
    *jar = kept.join("; ");
}

fn cookie_value(jar: &str, name: &str) -> Option<String> {
    jar.split(';').find_map(|part| {
        let (key, value) = part.trim().split_once('=')?;
        (key == name).then(|| value.to_string())
    })
}

#[tokio::test]
async fn search_filters_price_and_amenity() {
    let mut client = Client::new().await;
    let (status, body) = client.send("GET", "/api/search?destination=amalfi-coast", None, false).await;
    assert_eq!(status, StatusCode::OK);
    let json: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["total"], 2);

    let (status, body) = client.send("GET", "/api/search?minPrice=200", None, false).await;
    assert_eq!(status, StatusCode::OK);
    let json: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["total"], 1);
    assert_eq!(json["results"][0]["hotel"]["slug"], "casa-luna");

    let (status, body) = client.send("GET", "/api/search?amenities=amenity-pool", None, false).await;
    assert_eq!(status, StatusCode::OK);
    let json: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["total"], 1);
}

#[tokio::test]
async fn inquiry_requires_csrf_and_validates_email() {
    let mut client = Client::new().await;
    let (status, _) = client
        .send("POST", "/api/inquiries", Some(r#"{"hotelId":"hotel-casa","checkIn":"2026-10-01","checkOut":"2026-10-04","adults":2,"name":"Ada Lovelace","email":"ada@example.com"}"#), false)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = client.send("GET", "/api/csrf", None, false).await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = client
        .send("POST", "/api/inquiries", Some(r#"{"hotelId":"hotel-casa","checkIn":"2026-10-01","checkOut":"2026-10-04","adults":2,"name":"Ada Lovelace","email":"not-an-email"}"#), true)
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("email"));

    let (status, body) = client
        .send("POST", "/api/inquiries", Some(r#"{"hotelId":"hotel-casa","checkIn":"2026-10-01","checkOut":"2026-10-04","adults":2,"name":"Ada Lovelace","email":"ada@example.com","website":""}"#), true)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let json: Value = serde_json::from_str(&body).unwrap();
    assert!(json["reference"].as_str().unwrap().len() == 8);
}

#[tokio::test]
async fn publish_invalidates_snapshots() {
    let mut client = Client::new().await;
    sqlx::query("INSERT INTO page_snapshots (path, html, asset_version, generated_at) VALUES ('/hotels/casa-luna', '<html></html>', 'old', now())")
        .execute(&pool_from_env().await)
        .await
        .unwrap();
    let (status, _) = client.send("GET", "/api/csrf", None, false).await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = client
        .send("POST", "/api/admin/login", Some(r#"{"email":"admin@example.com","password":"ChangeMe123!"}"#), true)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, _) = client.send("GET", "/api/csrf", None, false).await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = client.send("POST", "/api/admin/hotels/hotel-casa/publish", Some(r#"{"status":"draft"}"#), true).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let pool = pool_from_env().await;
    let snapshot: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM page_snapshots WHERE path = '/hotels/casa-luna'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(snapshot, 0);
    let jobs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM snapshot_jobs WHERE path = '/'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(jobs >= 1);
}

#[tokio::test]
async fn snapshot_source_requires_token_and_renders_live_html() {
    let mut client = Client::new().await;
    let (status, _) = client.send("GET", "/_snapshot-source?path=/", None, false).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    sqlx::query("INSERT INTO snapshot_jobs (path, desired_version, attempts, next_attempt_at) VALUES ('/', 'v1', 0, now()) ON CONFLICT (path) DO NOTHING")
        .execute(&pool_from_env().await)
        .await
        .unwrap();
    let request = Request::builder()
        .uri("/_snapshot-source?path=/")
        .header("x-snapshot-token", "test-snapshot-token-with-32-characters-minimum")
        .body(Body::empty())
        .unwrap();
    let response = client.app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = String::from_utf8_lossy(&response.into_body().collect().await.unwrap().to_bytes()).into_owned();
    assert!(body.contains("data-island=\"SearchForm\""));
    assert!(body.contains("worth remembering"));
    let (status, robots) = client.send("GET", "/robots.txt", None, false).await;
    assert_eq!(status, StatusCode::OK);
    assert!(robots.contains("Sitemap:"));
}

async fn pool_from_env() -> sqlx::PgPool {
    let url = std::env::var("TEST_DATABASE_URL").unwrap();
    sqlx::postgres::PgPoolOptions::new().connect(&url).await.unwrap()
}
