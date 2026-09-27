use std::collections::{HashMap, HashSet};

use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::{header, HeaderMap, HeaderValue, Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::AppState;

pub async fn gate(State(state): State<AppState>, request: Request<Body>, next: Next) -> Response {
    let path = request.uri().path().to_string();
    let query = request.uri().query().unwrap_or("").to_string();
    if path == "/robots.txt" || path == "/sitemap.xml" || path == "/_snapshot-source" {
        return next.run(request).await;
    }
    if path == "/search" && !query.is_empty() {
        let mut response = next.run(request).await;
        response.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("private, no-store"));
        return response;
    }
    if !state.config.snapshot_mode || !canonical_shape(&path) || request.method() != axum::http::Method::GET && request.method() != axum::http::Method::HEAD {
        return next.run(request).await;
    }
    match serve_snapshot(&state.pool, &path).await {
        Ok(response) => response,
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

pub async fn robots(State(state): State<AppState>) -> Response {
    text(
        StatusCode::OK,
        "text/plain; charset=utf-8",
        "public, max-age=300",
        format!(
            "User-agent: *\nDisallow: /admin\nDisallow: /inquire\nDisallow: /_snapshot-source\nSitemap: {}/sitemap.xml\n",
            state.config.public_origin
        ),
    )
}

pub async fn sitemap(State(state): State<AppState>) -> Response {
    let paths = match sitemap_paths(&state.pool).await {
        Ok(paths) => paths,
        Err(error) => return (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    };
    let mut xml = String::from(r#"<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">"#);
    for path in paths {
        xml.push_str(&format!("<url><loc>{}{}</loc></url>", state.config.public_origin, escape_xml(&path)));
    }
    xml.push_str("</urlset>");
    text(StatusCode::OK, "application/xml; charset=utf-8", "public, max-age=300", xml)
}

pub async fn source(State(state): State<AppState>, headers: HeaderMap, Query(pairs): Query<Vec<(String, String)>>) -> Response {
    let supplied = headers.get("x-snapshot-token").and_then(|value| value.to_str().ok()).unwrap_or("");
    if state.config.snapshot_token.is_empty() || !crate::util::constant_time_eq(supplied, &state.config.snapshot_token) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let path = pairs.iter().find(|(key, _)| key == "path").map(|(_, value)| urlencoding_decode(value)).unwrap_or_default();
    if !canonical_shape(&path) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let known = sqlx::query_scalar::<_, i64>(
        "SELECT (SELECT COUNT(*) FROM snapshot_jobs WHERE path = $1) + (SELECT COUNT(*) FROM page_snapshots WHERE path = $1)",
    )
    .bind(&path)
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);
    if known == 0 {
        return StatusCode::NOT_FOUND.into_response();
    }
    let mut response = render_live(state, &path).await;
    response.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("private, no-store"));
    response.headers_mut().insert("x-robots-tag", HeaderValue::from_static("noindex, nofollow"));
    response
}

async fn render_live(state: AppState, path: &str) -> Response {
    if path == "/" {
        return crate::pages::home(State(state)).await;
    }
    if path == "/destinations" {
        return crate::pages::destinations(State(state)).await;
    }
    if path == "/blog" {
        return crate::pages::blog(State(state)).await;
    }
    if path == "/search" {
        return crate::pages::search_page(State(state), Query(Vec::new())).await;
    }
    if let Some(slug) = path.strip_prefix("/destinations/") {
        return crate::pages::destination(State(state), axum::extract::Path(slug.to_string())).await;
    }
    if let Some(slug) = path.strip_prefix("/blog/") {
        return crate::pages::post(State(state), axum::extract::Path(slug.to_string())).await;
    }
    if let Some(rest) = path.strip_prefix("/hotels/") {
        if let Some((hotel, offer)) = rest.split_once("/offers/") {
            return crate::pages::offer(State(state), axum::extract::Path((hotel.to_string(), offer.to_string()))).await;
        }
        return crate::pages::hotel(State(state), axum::extract::Path(rest.to_string())).await;
    }
    StatusCode::NOT_FOUND.into_response()
}

async fn serve_snapshot(pool: &PgPool, path: &str) -> Result<Response, sqlx::Error> {
    let html: Option<(String, String)> = sqlx::query_as("SELECT html, asset_version FROM page_snapshots WHERE path = $1")
        .bind(path)
        .fetch_optional(pool)
        .await?;
    if let Some((html, version)) = html {
        let mut response = text(StatusCode::OK, "text/html; charset=utf-8", "public, max-age=60, stale-while-revalidate=300", html);
        if let Ok(value) = HeaderValue::from_str(&version) {
            response.headers_mut().insert("x-snapshot-version", value);
        }
        return Ok(response);
    }
    let queued: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM snapshot_jobs WHERE path = $1)")
        .bind(path)
        .fetch_one(pool)
        .await?;
    let waiting = queued || matches!(path, "/" | "/destinations" | "/blog" | "/search");
    if waiting {
        let mut response = text(StatusCode::SERVICE_UNAVAILABLE, "text/plain; charset=utf-8", "no-store", "Page is being published. Please retry shortly.".into());
        response.headers_mut().insert(header::RETRY_AFTER, HeaderValue::from_static("5"));
        Ok(response)
    } else {
        Ok(text(StatusCode::NOT_FOUND, "text/plain; charset=utf-8", "no-store", "Page not found.".into()))
    }
}

pub fn canonical_shape(path: &str) -> bool {
    if matches!(path, "/" | "/destinations" | "/blog" | "/search") {
        return true;
    }
    if path.len() < 3 || path.len() > 300 {
        return false;
    }
    if let Some(slug) = path.strip_prefix("/destinations/") {
        return valid_slug(slug);
    }
    if let Some(slug) = path.strip_prefix("/blog/") {
        return valid_slug(slug);
    }
    let Some(rest) = path.strip_prefix("/hotels/") else {
        return false;
    };
    if let Some((hotel, offer)) = rest.split_once("/offers/") {
        valid_slug(hotel) && valid_slug(offer)
    } else {
        valid_slug(rest)
    }
}

fn valid_slug(slug: &str) -> bool {
    let bytes = slug.as_bytes();
    if bytes.len() < 1 || bytes.len() > 160 || bytes[0] == b'-' || bytes[bytes.len() - 1] == b'-' {
        return false;
    }
    bytes.iter().all(|character| character.is_ascii_lowercase() || character.is_ascii_digit() || *character == b'-')
}

struct Row {
    id: String,
    slug: String,
    status: String,
    destination_id: Option<String>,
    hotel_id: Option<String>,
}

pub async fn invalidate(
    tx: &mut Transaction<'_, Postgres>,
    kind: &str,
    id: &str,
    old_path: Option<&str>,
    old_destination_id: Option<&str>,
    old_hotel_id: Option<&str>,
) -> Result<(), sqlx::Error> {
    let destinations = load_rows(tx, "SELECT id, slug, status, NULL::text AS destination_id, NULL::text AS hotel_id FROM destinations").await?;
    let hotels = load_rows(tx, "SELECT id, slug, status, destination_id, NULL::text AS hotel_id FROM hotels").await?;
    let offers = load_rows(tx, "SELECT id, slug, status, NULL::text AS destination_id, hotel_id FROM offers").await?;
    let posts = load_rows(tx, "SELECT id, slug, status, NULL::text AS destination_id, NULL::text AS hotel_id FROM blog_posts").await?;
    let published = published_paths(&destinations, &hotels, &offers, &posts);
    let mut affected = HashSet::new();
    affected.insert("/".to_string());
    if let Some(old_path) = old_path {
        affected.insert(old_path.to_string());
    }
    let mut destination_ids = HashSet::new();
    let mut hotel_ids = HashSet::new();
    match kind {
        "destinations" => {
            destination_ids.insert(id.to_string());
            affected.insert("/destinations".into());
            affected.insert("/search".into());
        }
        "hotels" => {
            hotel_ids.insert(id.to_string());
            affected.insert("/search".into());
            affected.insert("/destinations".into());
            if let Some(old_destination_id) = old_destination_id {
                destination_ids.insert(old_destination_id.to_string());
            }
            if let Some(hotel) = hotels.iter().find(|row| row.id == id) {
                if let Some(destination_id) = &hotel.destination_id {
                    destination_ids.insert(destination_id.clone());
                }
            }
        }
        "rooms" | "offers" => {
            affected.insert("/search".into());
            if let Some(old_hotel_id) = old_hotel_id {
                hotel_ids.insert(old_hotel_id.to_string());
            }
            let hotel_id = if kind == "offers" {
                offers.iter().find(|row| row.id == id).and_then(|row| row.hotel_id.clone())
            } else {
                sqlx::query_scalar("SELECT hotel_id FROM rooms WHERE id = $1").bind(id).fetch_optional(&mut **tx).await?
            };
            if let Some(hotel_id) = hotel_id {
                hotel_ids.insert(hotel_id);
            }
        }
        "posts" => {
            affected.insert("/blog".into());
        }
        _ => {}
    }
    for destination_id in destination_ids.iter() {
        if let Some(destination) = destinations.iter().find(|row| row.id == *destination_id) {
            affected.insert(format!("/destinations/{}", destination.slug));
        }
        for hotel in hotels.iter().filter(|row| row.destination_id.as_deref() == Some(destination_id)) {
            hotel_ids.insert(hotel.id.clone());
        }
    }
    for hotel_id in &hotel_ids {
        let Some(hotel) = hotels.iter().find(|row| row.id == *hotel_id) else {
            continue;
        };
        affected.insert(format!("/hotels/{}", hotel.slug));
        affected.insert("/destinations".into());
        if let Some(destination_id) = &hotel.destination_id {
            if let Some(destination) = destinations.iter().find(|row| row.id == *destination_id) {
                affected.insert(format!("/destinations/{}", destination.slug));
            }
        }
        for offer in offers.iter().filter(|row| row.hotel_id.as_deref() == Some(hotel_id.as_str())) {
            affected.insert(format!("/hotels/{}/offers/{}", hotel.slug, offer.slug));
        }
    }
    if kind == "posts" {
        if let Some(post) = posts.iter().find(|row| row.id == id) {
            affected.insert(format!("/blog/{}", post.slug));
        }
    }
    if kind == "hotels" {
        if let Some(old_path) = old_path {
            let prefix = format!("{old_path}/offers/");
            let snapshots: Vec<String> = sqlx::query_scalar("SELECT path FROM page_snapshots WHERE path LIKE $1")
                .bind(format!("{prefix}%"))
                .fetch_all(&mut **tx)
                .await?;
            let jobs: Vec<String> = sqlx::query_scalar("SELECT path FROM snapshot_jobs WHERE path LIKE $1")
                .bind(format!("{prefix}%"))
                .fetch_all(&mut **tx)
                .await?;
            affected.extend(snapshots);
            affected.extend(jobs);
        }
    }
    if kind != "posts" {
        for post in posts.iter().filter(|row| row.status == "published") {
            affected.insert(format!("/blog/{}", post.slug));
        }
    }
    let version = Uuid::new_v4().simple().to_string();
    for path in affected {
        if !published.contains(&path) {
            sqlx::query("DELETE FROM page_snapshots WHERE path = $1").bind(&path).execute(&mut **tx).await?;
            sqlx::query("DELETE FROM snapshot_jobs WHERE path = $1").bind(&path).execute(&mut **tx).await?;
            continue;
        }
        sqlx::query(
            "INSERT INTO snapshot_jobs (path, desired_version, attempts, next_attempt_at, leased_until)
             VALUES ($1, $2, 0, now(), NULL)
             ON CONFLICT (path) DO UPDATE SET desired_version = EXCLUDED.desired_version, attempts = 0, next_attempt_at = now(), leased_until = NULL",
        )
        .bind(&path)
        .bind(&version)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

fn published_paths(destinations: &[Row], hotels: &[Row], offers: &[Row], posts: &[Row]) -> HashSet<String> {
    let published_destinations: HashMap<&str, &Row> = destinations.iter().filter(|row| row.status == "published").map(|row| (row.id.as_str(), row)).collect();
    let published_hotels: HashMap<&str, &Row> = hotels
        .iter()
        .filter(|row| row.status == "published" && row.destination_id.as_deref().is_some_and(|id| published_destinations.contains_key(id)))
        .map(|row| (row.id.as_str(), row))
        .collect();
    let mut paths = HashSet::from(["/".into(), "/destinations".into(), "/blog".into(), "/search".into()]);
    for destination in published_destinations.values() {
        paths.insert(format!("/destinations/{}", destination.slug));
    }
    for hotel in published_hotels.values() {
        paths.insert(format!("/hotels/{}", hotel.slug));
    }
    for offer in offers.iter().filter(|row| row.status == "published") {
        if let Some(hotel_id) = &offer.hotel_id {
            if let Some(hotel) = published_hotels.get(hotel_id.as_str()) {
                paths.insert(format!("/hotels/{}/offers/{}", hotel.slug, offer.slug));
            }
        }
    }
    for post in posts.iter().filter(|row| row.status == "published") {
        paths.insert(format!("/blog/{}", post.slug));
    }
    paths
}

async fn load_rows(tx: &mut Transaction<'_, Postgres>, sql: &str) -> Result<Vec<Row>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, String, Option<String>, Option<String>)>(sql)
        .fetch_all(&mut **tx)
        .await?;
    Ok(rows
        .into_iter()
        .map(|(id, slug, status, destination_id, hotel_id)| Row { id, slug, status, destination_id, hotel_id })
        .collect())
}

pub async fn sitemap_paths(pool: &PgPool) -> Result<Vec<String>, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let destinations = load_rows(&mut tx, "SELECT id, slug, status, NULL::text, NULL::text FROM destinations").await?;
    let hotels = load_rows(&mut tx, "SELECT id, slug, status, destination_id, NULL::text FROM hotels").await?;
    let offers = load_rows(&mut tx, "SELECT id, slug, status, NULL::text, hotel_id FROM offers").await?;
    let posts = load_rows(&mut tx, "SELECT id, slug, status, NULL::text, NULL::text FROM blog_posts").await?;
    let mut paths: Vec<String> = published_paths(&destinations, &hotels, &offers, &posts).into_iter().collect();
    paths.sort();
    Ok(paths)
}

pub async fn queue_missing(pool: &PgPool) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    let destinations = load_rows(&mut tx, "SELECT id, slug, status, NULL::text, NULL::text FROM destinations").await?;
    let hotels = load_rows(&mut tx, "SELECT id, slug, status, destination_id, NULL::text FROM hotels").await?;
    let offers = load_rows(&mut tx, "SELECT id, slug, status, NULL::text, hotel_id FROM offers").await?;
    let posts = load_rows(&mut tx, "SELECT id, slug, status, NULL::text, NULL::text FROM blog_posts").await?;
    let version = Uuid::new_v4().simple().to_string();
    for path in published_paths(&destinations, &hotels, &offers, &posts) {
        sqlx::query(
            "INSERT INTO snapshot_jobs (path, desired_version, attempts, next_attempt_at, leased_until)
             SELECT $1, $2, 0, now(), NULL
             WHERE NOT EXISTS (SELECT 1 FROM page_snapshots WHERE path = $1)
             ON CONFLICT (path) DO NOTHING",
        )
        .bind(&path)
        .bind(&version)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await
}

fn text(status: StatusCode, content_type: &str, cache: &'static str, body: String) -> Response {
    let mut response = (status, body).into_response();
    response.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_str(content_type).unwrap());
    response.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    response
}

fn escape_xml(value: &str) -> String {
    value.replace('&', "&amp;").replace('<', "&lt;")
}

fn urlencoding_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(std::str::from_utf8(&bytes[index + 1..index + 3]).unwrap_or(""), 16) {
                out.push(byte);
                index += 3;
                continue;
            }
        }
        if bytes[index] == b'+' {
            out.push(b' ');
        } else {
            out.push(bytes[index]);
        }
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
