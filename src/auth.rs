use axum::http::HeaderMap;
use axum_extra::extract::cookie::{Cookie, SameSite};
use chrono::{Duration, Utc};
use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::config::Config;
use crate::util::constant_time_eq;

type HmacSha256 = Hmac<Sha256>;

pub fn session_cookie(config: &Config, user_id: &str) -> Cookie<'static> {
    let expiry = (Utc::now() + Duration::hours(12)).timestamp();
    let payload = format!("{user_id}.{expiry}");
    let token = format!("{payload}.{}", sign(config, &payload));
    cookie(config, "hotel-admin", token, true)
}

pub fn read_session(config: &Config, header: &str) -> Option<String> {
    let token = cookie_value(header, "hotel-admin")?;
    let mut parts = token.split('.');
    let user_id = parts.next()?;
    let expiry = parts.next()?;
    let signature = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    let payload = format!("{user_id}.{expiry}");
    if !constant_time_eq(&sign(config, &payload), signature) {
        return None;
    }
    let expiry: i64 = expiry.parse().ok()?;
    if expiry < Utc::now().timestamp() {
        return None;
    }
    Some(user_id.to_string())
}

pub fn csrf_cookie(config: &Config) -> (Cookie<'static>, String) {
    let token = hex::encode(rand::random::<[u8; 32]>());
    (cookie(config, "hotel-csrf", token.clone(), true), token)
}

pub fn csrf_matches(headers: &HeaderMap, cookie_header: Option<&str>) -> bool {
    let Some(header) = headers.get("x-csrf-token").and_then(|value| value.to_str().ok()) else {
        return false;
    };
    let Some(cookie_header) = cookie_header else {
        return false;
    };
    let Some(cookie) = cookie_value(cookie_header, "hotel-csrf") else {
        return false;
    };
    constant_time_eq(header, &cookie)
}

pub fn clear_cookie(config: &Config, name: &'static str) -> Cookie<'static> {
    let mut cookie = Cookie::build((name, "")).path("/").same_site(SameSite::Lax).http_only(true).max_age(cookie::time::Duration::seconds(0)).build();
    if config.secure_cookies {
        cookie.set_secure(true);
    }
    cookie
}

fn cookie(config: &Config, name: &'static str, value: String, http_only: bool) -> Cookie<'static> {
    let mut cookie = Cookie::build((name, value))
        .path("/")
        .same_site(SameSite::Lax)
        .http_only(http_only)
        .max_age(cookie::time::Duration::hours(12))
        .build();
    if config.secure_cookies {
        cookie.set_secure(true);
    }
    cookie
}

fn sign(config: &Config, payload: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(config.session_secret.as_bytes()).expect("session secret");
    mac.update(payload.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

fn cookie_value(header: &str, name: &str) -> Option<String> {
    for part in header.split(';') {
        let mut pair = part.trim().splitn(2, '=');
        if pair.next() == Some(name) {
            return pair.next().map(|value| value.to_string());
        }
    }
    None
}
