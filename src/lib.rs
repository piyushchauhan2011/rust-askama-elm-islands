use std::sync::Arc;

use axum::body::Body;
use axum::extract::State;
use axum::http::{header, HeaderValue, Request};
use axum::middleware::Next;
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::json;
use sqlx::PgPool;
use tower_http::compression::CompressionLayer;
use tower_http::services::ServeDir;

use crate::assets::Assets;
use crate::catalog::ImageIndex;

pub use config::Config;

pub mod admin;
pub mod api;
pub mod assets;
pub mod auth;
pub mod catalog;
pub mod config;
pub mod inquiry;
pub mod models;
pub mod pages;
pub mod seed;
pub mod snapshots;
pub mod util;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Config,
    pub assets: Arc<Assets>,
    pub images: Arc<ImageIndex>,
}

pub fn router(state: AppState) -> Router {
    let shared = state.clone();
    Router::new()
        .route("/health", get(|| async { Json(json!({"status": "ok"})) }))
        .route("/robots.txt", get(snapshots::robots))
        .route("/sitemap.xml", get(snapshots::sitemap))
        .route("/_snapshot-source", get(snapshots::source))
        .route("/", get(pages::home))
        .route("/destinations", get(pages::destinations))
        .route("/destinations/{slug}", get(pages::destination))
        .route("/hotels/{slug}", get(pages::hotel))
        .route("/hotels/{slug}/offers/{offer_slug}", get(pages::offer))
        .route("/blog", get(pages::blog))
        .route("/blog/{slug}", get(pages::post))
        .route("/search", get(pages::search_page))
        .route("/inquire", get(pages::inquire))
        .route("/admin", get(pages::admin_page))
        .route("/admin/{*rest}", get(pages::admin_page))
        .route("/api/home", get(api::home))
        .route("/api/destinations", get(api::destinations))
        .route("/api/destinations/{slug}", get(api::destination))
        .route("/api/hotels/{slug}", get(api::hotel))
        .route("/api/hotels/{slug}/offers/{offer_slug}", get(api::offer))
        .route("/api/posts", get(api::posts))
        .route("/api/posts/{slug}", get(api::post))
        .route("/api/search", get(api::search))
        .route("/api/inquiry-context", get(api::inquiry_context))
        .route("/api/inquiries", post(api::submit_inquiry))
        .route("/api/csrf", get(admin::csrf))
        .route("/api/admin/login", post(admin::login))
        .route("/api/admin/logout", post(admin::logout))
        .route("/api/admin/me", get(admin::me))
        .route("/api/admin/dashboard", get(admin::dashboard))
        .route("/api/admin/inquiries", get(admin::inquiries))
        .route("/api/admin/inquiries/{id}/status", post(admin::inquiry_status))
        .route("/api/admin/media", post(admin::upload_media))
        .route("/media/{id}/{variant}", get(admin::media))
        .route("/api/admin/{kind}", get(admin::list).post(admin::save))
        .route("/api/admin/{kind}/{id}", get(admin::item))
        .route("/api/admin/{kind}/{id}/publish", post(admin::publish))
        .nest_service("/assets", ServeDir::new("static/assets"))
        .nest_service("/images", ServeDir::new("static/images"))
        .route_service("/favicon.svg", ServeDir::new("static").append_index_html_on_directories(false))
        .layer(axum::middleware::from_fn(static_cache))
        .layer(axum::middleware::from_fn_with_state(shared, snapshots::gate))
        .layer(CompressionLayer::new())
        .with_state(state)
}

async fn static_cache(request: Request<Body>, next: Next) -> Response {
    let path = request.uri().path().to_string();
    let mut response = next.run(request).await;
    let cache = if path.starts_with("/assets/") || path.starts_with("/images/gen/") {
        Some("public, max-age=31536000, immutable")
    } else if path.starts_with("/images/") || path == "/favicon.svg" {
        Some("public, max-age=86400")
    } else {
        None
    };
    if let Some(value) = cache {
        response.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static(value));
    }
    response
}

pub async fn connect(config: &Config) -> Result<PgPool, sqlx::Error> {
    let pool = sqlx::postgres::PgPoolOptions::new().max_connections(10).connect(&config.database_url).await?;
    sqlx::migrate!().run(&pool).await?;
    Ok(pool)
}

pub fn state(pool: PgPool, config: Config) -> AppState {
    AppState {
        pool,
        assets: Arc::new(Assets::load(config.asset_mode_vite, &config.vite_origin)),
        images: Arc::new(ImageIndex::load()),
        config,
    }
}

#[allow(dead_code)]
async fn health_state(State(_state): State<AppState>) -> Json<serde_json::Value> {
    Json(json!({"status": "ok"}))
}
