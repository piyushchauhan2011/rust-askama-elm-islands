use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};

use crate::auth;
use crate::catalog::{self, SearchFilters};
use crate::inquiry;
use crate::AppState;

pub async fn home(State(state): State<AppState>) -> Response {
    match catalog::home(&state.pool).await {
        Ok((destinations, hotels, _, posts)) => {
            let offers = catalog::home_offers(&state.pool).await.unwrap_or_default();
            Json(json!({
                "destinations": destinations,
                "hotels": hotels,
                "offers": offers.into_iter().map(|(offer, hotel_slug)| {
                    let mut value = serde_json::to_value(offer).unwrap_or(Value::Null);
                    if let Some(object) = value.as_object_mut() {
                        object.insert("hotelSlug".into(), Value::String(hotel_slug));
                    }
                    value
                }).collect::<Vec<_>>(),
                "posts": posts,
            })).into_response()
        }
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

pub async fn destinations(State(state): State<AppState>) -> Response {
    json_result(catalog::destinations(&state.pool).await)
}

pub async fn destination(State(state): State<AppState>, Path(slug): Path<String>) -> Response {
    match catalog::destination(&state.pool, &slug).await {
        Ok(Some((destination, hotels))) => Json(json!({"destination": destination, "hotels": hotels})).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

pub async fn hotel(State(state): State<AppState>, Path(slug): Path<String>) -> Response {
    match catalog::hotel(&state.pool, &slug).await {
        Ok(Some(detail)) => {
            let mut groups: Vec<Value> = Vec::new();
            let mut current = String::new();
            for amenity in &detail.amenities {
                if amenity.category != current {
                    current = amenity.category.clone();
                    groups.push(json!({"category": current, "amenities": []}));
                }
                if let Some(group) = groups.last_mut().and_then(|group| group.get_mut("amenities")).and_then(Value::as_array_mut) {
                    group.push(serde_json::to_value(amenity).unwrap_or(Value::Null));
                }
            }
            Json(json!({
                "hotel": detail.hotel,
                "destination": detail.destination,
                "rooms": detail.rooms,
                "offers": detail.offers,
                "amenities": detail.amenities,
                "roomAmenities": detail.room_amenities,
                "gallery": detail.gallery,
                "highlights": detail.highlights,
                "facts": detail.facts,
                "faqs": detail.faqs,
                "nearby": detail.nearby,
                "policies": detail.policies,
                "reviewScores": detail.review_scores,
                "reviews": detail.reviews,
                "amenitiesByCategory": groups,
            })).into_response()
        }
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

pub async fn offer(State(state): State<AppState>, Path((slug, offer_slug)): Path<(String, String)>) -> Response {
    match catalog::offer(&state.pool, &slug, &offer_slug).await {
        Ok(Some((offer, hotel, destination))) => Json(json!({"offer": offer, "hotel": hotel, "destination": destination})).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

pub async fn posts(State(state): State<AppState>) -> Response {
    json_result(catalog::posts(&state.pool).await)
}

pub async fn post(State(state): State<AppState>, Path(slug): Path<String>) -> Response {
    match catalog::post(&state.pool, &slug).await {
        Ok(Some((post, hotels))) => {
            let embedded = hotels.into_iter().map(|(id, hotel)| (id, serde_json::to_value(hotel).unwrap_or(Value::Null))).collect::<serde_json::Map<_, _>>();
            Json(json!({"post": post, "embeddedHotels": embedded})).into_response()
        }
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

pub async fn search(State(state): State<AppState>, Query(pairs): Query<Vec<(String, String)>>) -> Response {
    let filters = SearchFilters::from_pairs(&pairs);
    match catalog::search(&state.pool, &filters).await {
        Ok(page) => Json(json!({
            "results": page.results.into_iter().map(|hit| json!({"hotel": hit.hotel, "destination": hit.destination})).collect::<Vec<_>>(),
            "total": page.total,
            "page": filters.page,
            "pages": page.pages,
            "filters": {
                "destination": filters.destination,
                "query": filters.query,
                "checkIn": filters.check_in,
                "checkOut": filters.check_out,
                "adults": filters.adults,
                "children": filters.children,
                "minPrice": filters.min_price,
                "maxPrice": filters.max_price,
                "rating": filters.rating,
                "amenities": filters.amenities,
                "offers": filters.offers,
                "sort": filters.sort,
                "page": filters.page,
            },
            "amenities": page.amenities,
        })).into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

pub async fn inquiry_context(State(state): State<AppState>, Query(pairs): Query<Vec<(String, String)>>) -> Response {
    let value = |key: &str| pairs.iter().find(|(name, _)| name == key).map(|(_, item)| item.clone());
    let Some(hotel_id) = value("hotel").filter(|item| !item.is_empty()) else {
        return (StatusCode::BAD_REQUEST, Json(json!({"fieldErrors": {"hotel": ["Hotel is required"]}}))).into_response();
    };
    match catalog::inquiry_context(&state.pool, &hotel_id, value("room").as_deref(), value("offer").as_deref()).await {
        Ok(Some((hotel, room, offer))) => Json(json!({
            "id": hotel.id,
            "destinationId": hotel.destination_id,
            "name": hotel.name,
            "slug": hotel.slug,
            "propertyType": hotel.property_type,
            "address": hotel.address,
            "summary": hotel.summary,
            "description": hotel.description,
            "heroImage": hotel.hero_image,
            "rating": hotel.rating,
            "starRating": hotel.star_rating,
            "reviewCount": hotel.review_count,
            "priceFrom": hotel.price_from,
            "currency": hotel.currency,
            "selectedRoom": room,
            "selectedOffer": offer,
        })).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

pub async fn submit_inquiry(State(state): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    if !auth::csrf_matches(&headers, headers.get("cookie").and_then(|value| value.to_str().ok())) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "Invalid CSRF token"}))).into_response();
    }
    let Ok(payload) = serde_json::from_slice::<Value>(&body) else {
        return (StatusCode::BAD_REQUEST, Json(json!({"fieldErrors": {"body": ["Valid JSON object is required"]}}))).into_response();
    };
    match inquiry::submit(&state.pool, &payload).await {
        Ok(result) if result.errors.is_empty() => {
            Json(inquiry::success_body(result.id.as_deref().unwrap_or(""))).into_response()
        }
        Ok(result) => (StatusCode::BAD_REQUEST, Json(json!({"fieldErrors": result.errors}))).into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

fn json_result<T: serde::Serialize>(result: Result<T, sqlx::Error>) -> Response {
    match result {
        Ok(value) => Json(value).into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}
