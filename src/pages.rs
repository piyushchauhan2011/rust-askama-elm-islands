use std::collections::BTreeMap;

use askama::Template;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum_extra::extract::cookie::CookieJar;
use chrono::{Datelike, NaiveDate, Utc};
use serde::Serialize;
use serde_json::json;

use crate::auth;
use crate::catalog::{self, ImageIndex, SearchFilters};
use crate::models::*;
use crate::util::{price, rating};
use crate::AppState;

const ISLANDS_CSS: &str = include_str!("../static/assets/islands-inline.css");
const ADMIN_CSS: &str = include_str!("../static/assets/admin-inline.css");

struct Meta {
    seo_title: String,
    seo_description: String,
    canonical: String,
    robots: String,
    json_ld: String,
    header_overlay: bool,
    admin: bool,
    year: i32,
    css: &'static str,
    script: String,
}

struct PostCard {
    slug: String,
    title: String,
    excerpt: String,
    hero_image: String,
}

struct HiddenField {
    name: String,
    value: String,
}

struct PageLink {
    number: i32,
    href: String,
    current: bool,
}

struct NamedGroup {
    name: String,
    items: Vec<String>,
}

struct NearbyGroup {
    name: String,
    places: Vec<NearbyView>,
}

struct NearbyView {
    name: String,
    distance: String,
}

struct PolicyGroup {
    name: String,
    policies: Vec<PolicyView>,
}

#[derive(Clone)]
struct PolicyView {
    title: String,
    description: String,
}

struct RoomView {
    name: String,
    summary: String,
    image: String,
    max_guests: i32,
    size: String,
    bed: String,
    amenities: Vec<String>,
    has_gallery: bool,
    price: String,
    inquire: String,
}

struct ScoreView {
    label: String,
    score: String,
    value: String,
    width: String,
}

struct ReviewView {
    rating: String,
    reviewed_at: String,
    title: String,
    body: String,
    guest: String,
    stay: String,
    room: String,
    response: String,
}

struct HighlightView {
    title: String,
    summary: String,
}

struct FactView {
    label: String,
    value: String,
}

struct FaqView {
    question: String,
    answer: String,
}

#[derive(Template)]
#[template(path = "home.html")]
struct HomeTemplate<'a> {
    meta: &'a Meta,
    search_props: String,
    destination_cards: Vec<String>,
    hotel_cards: Vec<String>,
    offer_cards: Vec<String>,
    posts: Vec<PostCard>,
}

#[derive(Template)]
#[template(path = "destinations.html")]
struct DestinationsTemplate<'a> {
    meta: &'a Meta,
    cards: Vec<String>,
}

#[derive(Template)]
#[template(path = "destination.html")]
struct DestinationTemplate<'a> {
    meta: &'a Meta,
    name: String,
    name_upper: String,
    eyebrow: String,
    country: String,
    summary: String,
    hero_image: String,
    search_props: String,
    rich: String,
    hotel_cards: Vec<String>,
}

#[derive(Template)]
#[template(path = "blog.html")]
struct BlogTemplate<'a> {
    meta: &'a Meta,
    posts: Vec<PostCard>,
}

#[derive(Template)]
#[template(path = "post.html")]
struct PostTemplate<'a> {
    meta: &'a Meta,
    title: String,
    excerpt: String,
    author: String,
    hero_image: String,
    rich: String,
}

#[derive(Template)]
#[template(path = "search.html")]
struct SearchTemplate<'a> {
    meta: &'a Meta,
    search_props: String,
    filter_props: String,
    total: i64,
    sort: String,
    hidden: Vec<HiddenField>,
    cards: Vec<String>,
    pages: i32,
    page_links: Vec<PageLink>,
}

#[derive(Template)]
#[template(path = "inquire.html")]
struct InquireTemplate<'a> {
    meta: &'a Meta,
    hero_image: String,
    hotel_name: String,
    address: String,
    room_name: String,
    offer_title: String,
    inquiry_props: String,
}

#[derive(Template)]
#[template(path = "hotel.html")]
struct HotelTemplate<'a> {
    meta: &'a Meta,
    name: String,
    destination_name: String,
    destination_slug: String,
    destination_summary: String,
    country: String,
    property_type: String,
    summary: String,
    description: String,
    address: String,
    rating_line: String,
    rating: String,
    price: String,
    inquire_url: String,
    gallery_props: String,
    map_url: String,
    review_count_label: String,
    show_reviews: bool,
    highlights: Vec<HighlightView>,
    rooms: Vec<RoomView>,
    amenity_groups: Vec<NamedGroup>,
    facts: Vec<FactView>,
    nearby_groups: Vec<NearbyGroup>,
    policy_groups: Vec<PolicyGroup>,
    policies: Vec<PolicyView>,
    faqs: Vec<FaqView>,
    scores: Vec<ScoreView>,
    reviews: Vec<ReviewView>,
    offer_cards: Vec<String>,
}

#[derive(Template)]
#[template(path = "offer.html")]
struct OfferTemplate<'a> {
    meta: &'a Meta,
    title: String,
    summary: String,
    image: String,
    hotel_name: String,
    hotel_slug: String,
    destination_name: String,
    destination_slug: String,
    discount_label: String,
    benefit: String,
    validity: String,
    terms: String,
    inquiry_url: String,
}

#[derive(Template)]
#[template(path = "admin.html")]
struct AdminTemplate<'a> {
    meta: &'a Meta,
}

#[derive(Template)]
#[template(path = "missing.html")]
struct MissingTemplate<'a> {
    meta: &'a Meta,
}

fn meta(state: &AppState, title: &str, description: &str, path: &str, robots: &str, overlay: bool, admin: bool, json_ld: String) -> Meta {
    Meta {
        seo_title: title.to_string(),
        seo_description: description.to_string(),
        canonical: format!("{}{path}", state.config.public_origin),
        robots: robots.to_string(),
        json_ld,
        header_overlay: overlay,
        admin,
        year: Utc::now().year(),
        css: if admin { ADMIN_CSS } else { ISLANDS_CSS },
        script: if admin { state.assets.admin_script.clone() } else { state.assets.runtime_script.clone() },
    }
}

fn render(template: impl Template, cache: &'static str, status: StatusCode) -> Response {
    match template.render() {
        Ok(body) => {
            let mut response = (status, Html(body)).into_response();
            if let Ok(value) = HeaderValue::from_str(cache) {
                response.headers_mut().insert(header::CACHE_CONTROL, value);
            }
            response
        }
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

fn missing(state: &AppState) -> Response {
    let meta = meta(state, "Not found — Elsewhere", "This page is not published.", "/", "noindex,nofollow", false, false, String::new());
    render(MissingTemplate { meta: &meta }, "no-store", StatusCode::NOT_FOUND)
}

pub async fn home(State(state): State<AppState>) -> Response {
    let Ok((destinations, hotels, _, posts)) = catalog::home(&state.pool).await else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let Ok(offers) = catalog::home_offers(&state.pool).await else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let meta = meta(&state, "Elsewhere — stays worth remembering", "Independent hotels and slower journeys, selected with care.", "/", "index,follow", true, false, String::new());
    let search_props = search_form_props(&destinations, None, None, None, None, 2, 0, false);
    render(
        HomeTemplate {
            meta: &meta,
            search_props,
            destination_cards: destinations.iter().map(|item| catalog::destination_card(item, &state.images)).collect(),
            hotel_cards: hotels.iter().map(|item| catalog::hotel_card(item, &state.images)).collect(),
            offer_cards: offers.iter().map(|(offer, slug)| catalog::offer_card(offer, slug, &state.images)).collect(),
            posts: posts.into_iter().map(post_card).collect(),
        },
        "private, no-store",
        StatusCode::OK,
    )
}

pub async fn destinations(State(state): State<AppState>) -> Response {
    let Ok(items) = catalog::destinations(&state.pool).await else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let meta = meta(&state, "Destinations — Elsewhere", "Travel guides and independent hotels in places worth knowing slowly.", "/destinations", "index,follow", false, false, String::new());
    render(
        DestinationsTemplate {
            meta: &meta,
            cards: items.iter().map(|item| catalog::destination_card(item, &state.images)).collect(),
        },
        "private, no-store",
        StatusCode::OK,
    )
}

pub async fn destination(State(state): State<AppState>, Path(slug): Path<String>) -> Response {
    let Ok(found) = catalog::destination(&state.pool, &slug).await else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let Some((destination, hotels)) = found else {
        return missing(&state);
    };
    let path = format!("/destinations/{}", destination.slug);
    let meta = meta(&state, &destination.seo_title, &destination.seo_description, &path, "index,follow", true, false, String::new());
    let search_props = search_form_props(std::slice::from_ref(&destination), Some(&destination.slug), None, None, None, 2, 0, false);
    render(
        DestinationTemplate {
            meta: &meta,
            name_upper: destination.name.to_uppercase(),
            name: destination.name.clone(),
            eyebrow: destination.eyebrow.clone(),
            country: destination.country.clone(),
            summary: destination.summary.clone(),
            hero_image: destination.hero_image.clone(),
            search_props,
            rich: catalog::render_rich(&destination.content, None, &state.images),
            hotel_cards: hotels.iter().map(|hotel| catalog::hotel_card(hotel, &state.images)).collect(),
        },
        "private, no-store",
        StatusCode::OK,
    )
}

pub async fn blog(State(state): State<AppState>) -> Response {
    let Ok(posts) = catalog::posts(&state.pool).await else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let meta = meta(&state, "The Journal — Elsewhere", "Field notes, hotel stories, and thoughtful guides for going well.", "/blog", "index,follow", false, false, String::new());
    render(BlogTemplate { meta: &meta, posts: posts.into_iter().map(post_card).collect() }, "private, no-store", StatusCode::OK)
}

pub async fn post(State(state): State<AppState>, Path(slug): Path<String>) -> Response {
    let Ok(found) = catalog::post(&state.pool, &slug).await else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let Some((post, hotels)) = found else {
        return missing(&state);
    };
    let path = format!("/blog/{}", post.slug);
    let json_ld = serde_json::to_string(&json!({
        "@context": "https://schema.org",
        "@type": "BlogPosting",
        "headline": post.title,
        "description": post.excerpt,
        "author": {"@type": "Person", "name": post.author},
        "image": post.hero_image,
        "datePublished": post.published_at.map(|value| value.to_string()),
    })).unwrap_or_default();
    let meta = meta(&state, &post.seo_title, &post.seo_description, &path, "index,follow", false, false, json_ld);
    render(
        PostTemplate {
            meta: &meta,
            title: post.title.clone(),
            excerpt: post.excerpt.clone(),
            author: post.author.clone(),
            hero_image: post.hero_image.clone(),
            rich: catalog::render_rich(&post.content, Some(&hotels), &state.images),
        },
        "private, no-store",
        StatusCode::OK,
    )
}

pub async fn hotel(State(state): State<AppState>, Path(slug): Path<String>) -> Response {
    let Ok(found) = catalog::hotel(&state.pool, &slug).await else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let Some(detail) = found else {
        return missing(&state);
    };
    let hotel = &detail.hotel;
    let path = format!("/hotels/{}", hotel.slug);
    let mut json = json!({
        "@context": "https://schema.org",
        "@type": "Hotel",
        "name": hotel.name,
        "description": hotel.summary,
        "image": hotel.hero_image,
        "address": hotel.address,
    });
    if let Some(stars) = hotel.star_rating {
        json["starRating"] = json!({"@type": "Rating", "ratingValue": stars});
    }
    if hotel.review_count > 0 {
        json["aggregateRating"] = json!({"@type": "AggregateRating", "ratingValue": hotel.rating, "reviewCount": hotel.review_count});
    }
    let meta = meta(&state, &hotel.seo_title, &hotel.seo_description, &path, "index,follow", false, false, serde_json::to_string(&json).unwrap_or_default());
    let review_count = hotel.review_count.max(detail.reviews.len() as i32);
    let mut rating_line = if let Some(stars) = hotel.star_rating {
        format!("{stars}-star · {} / 5", rating(hotel.rating))
    } else {
        format!("{} / 5", rating(hotel.rating))
    };
    if review_count > 0 {
        rating_line.push_str(&format!(" · {review_count} reviews"));
    }
    let gallery_props = serde_json::to_string(&json!({
        "name": hotel.name,
        "items": detail.gallery.iter().map(|item| gallery_item(item, &state.images)).collect::<Vec<_>>(),
    })).unwrap_or_else(|_| "{}".into());
    let mut amenity_groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for amenity in &detail.amenities {
        amenity_groups.entry(amenity.category.clone()).or_default().push(amenity.name.clone());
    }
    let mut nearby_groups: BTreeMap<String, Vec<NearbyView>> = BTreeMap::new();
    for place in &detail.nearby {
        let distance = if place.distance_meters < 1000 {
            format!("{} m", place.distance_meters)
        } else {
            format!("{:.1} km", place.distance_meters as f64 / 1000.0)
        };
        nearby_groups.entry(place.category.clone()).or_default().push(NearbyView { name: place.name.clone(), distance });
    }
    let mut policy_groups: BTreeMap<String, Vec<PolicyView>> = BTreeMap::new();
    for policy in &detail.policies {
        policy_groups.entry(policy.category.clone()).or_default().push(PolicyView { title: policy.title.clone(), description: policy.description.clone() });
    }
    let rooms = detail.rooms.iter().map(|room| RoomView {
        name: room.name.clone(),
        summary: room.summary.clone(),
        image: room.image.clone(),
        max_guests: room.max_guests,
        size: room.size_sqm.map(|size| format!("{size} m²")).unwrap_or_default(),
        bed: room.bed.clone(),
        amenities: detail.room_amenities.iter().filter(|item| item.room_id == room.id).map(|item| item.name.clone()).collect(),
        has_gallery: detail.gallery.iter().any(|photo| photo.room_id.as_deref() == Some(room.id.as_str())),
        price: price(room.price_from, &hotel.currency),
        inquire: format!("/inquire?hotel={}&room={}", catalog::urlencoding_escape(&hotel.id), catalog::urlencoding_escape(&room.id)),
    }).collect();
    render(
        HotelTemplate {
            meta: &meta,
            name: hotel.name.clone(),
            destination_name: detail.destination.name.clone(),
            destination_slug: detail.destination.slug.clone(),
            destination_summary: detail.destination.summary.clone(),
            country: detail.destination.country.clone(),
            property_type: hotel.property_type.clone(),
            summary: hotel.summary.clone(),
            description: hotel.description.clone(),
            address: hotel.address.clone(),
            rating_line,
            rating: rating(hotel.rating),
            price: price(hotel.price_from, &hotel.currency),
            inquire_url: format!("/inquire?hotel={}", catalog::urlencoding_escape(&hotel.id)),
            gallery_props,
            map_url: match (hotel.latitude, hotel.longitude) {
                (Some(latitude), Some(longitude)) => format!("https://www.google.com/maps?q={latitude},{longitude}"),
                _ => String::new(),
            },
            review_count_label: if review_count > 0 { format!("{review_count} reviews") } else { "No reviews yet".into() },
            show_reviews: !detail.review_scores.is_empty() || !detail.reviews.is_empty() || hotel.review_count > 0,
            highlights: detail.highlights.iter().map(|item| HighlightView { title: item.title.clone(), summary: item.summary.clone() }).collect(),
            rooms,
            amenity_groups: amenity_groups.into_iter().map(|(name, items)| NamedGroup { name, items }).collect(),
            facts: detail.facts.iter().map(|fact| FactView { label: fact.label.clone(), value: fact.value.clone() }).collect(),
            nearby_groups: nearby_groups.into_iter().map(|(name, places)| NearbyGroup { name, places }).collect(),
            policy_groups: policy_groups.clone().into_iter().map(|(name, policies)| PolicyGroup { name, policies }).collect(),
            policies: detail.policies.iter().map(|policy| PolicyView { title: policy.title.clone(), description: policy.description.clone() }).collect(),
            faqs: detail.faqs.iter().map(|faq| FaqView { question: faq.question.clone(), answer: faq.answer.clone() }).collect(),
            scores: detail.review_scores.iter().map(|score| ScoreView {
                label: score.label.clone(),
                score: rating(score.score),
                value: rating(score.score),
                width: format!("{:.0}", (score.score / 5.0 * 100.0).clamp(0.0, 100.0)),
            }).collect(),
            reviews: detail.reviews.iter().take(3).map(|review| ReviewView {
                rating: rating(review.rating),
                reviewed_at: review.reviewed_at.clone(),
                title: review.title.clone(),
                body: review.body.clone(),
                guest: format!("{} · {}", review.guest_name, review.guest_country),
                stay: format!("{} · {} {} · Stayed {}", review.traveler_type, review.nights, if review.nights == 1 { "night" } else { "nights" }, review.stayed_at),
                room: detail.rooms.iter().find(|room| Some(room.id.as_str()) == review.room_id.as_deref()).map(|room| room.name.clone()).unwrap_or_default(),
                response: review.response.clone().unwrap_or_default(),
            }).collect(),
            offer_cards: detail.offers.iter().map(|offer| catalog::offer_card(offer, &hotel.slug, &state.images)).collect(),
        },
        "private, no-store",
        StatusCode::OK,
    )
}

pub async fn offer(State(state): State<AppState>, Path((slug, offer_slug)): Path<(String, String)>) -> Response {
    let Ok(found) = catalog::offer(&state.pool, &slug, &offer_slug).await else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let Some((offer, hotel, destination)) = found else {
        return missing(&state);
    };
    let path = format!("/hotels/{}/offers/{}", hotel.slug, offer.slug);
    let meta = meta(&state, &format!("{} at {} — Elsewhere", offer.title, hotel.name), &offer.summary, &path, "index,follow", false, false, String::new());
    let validity = match (&offer.valid_from, &offer.valid_to) {
        (Some(from), Some(to)) => format!("Valid for stays from {} to {}.", pretty_date(from), pretty_date(to)),
        (Some(from), None) => format!("Valid for stays from {}.", pretty_date(from)),
        (None, Some(to)) => format!("Valid for stays until {}.", pretty_date(to)),
        _ => "Available on selected dates, subject to availability.".into(),
    };
    let discount = offer.discount_percent.filter(|value| *value > 0);
    render(
        OfferTemplate {
            meta: &meta,
            title: offer.title.clone(),
            summary: offer.summary.clone(),
            image: offer.image.clone(),
            hotel_name: hotel.name.clone(),
            hotel_slug: hotel.slug.clone(),
            destination_name: destination.name,
            destination_slug: destination.slug,
            discount_label: discount.map(|value| format!("{value}% value")).unwrap_or_else(|| "Seasonal value".into()),
            benefit: discount.map(|value| format!("{value}% value applied to this special stay.")).unwrap_or_else(|| "A special seasonal value created for this stay.".into()),
            validity,
            terms: offer.terms.clone(),
            inquiry_url: format!("/inquire?hotel={}&offer={}", catalog::urlencoding_escape(&hotel.id), catalog::urlencoding_escape(&offer.id)),
        },
        "private, no-store",
        StatusCode::OK,
    )
}

pub async fn search_page(State(state): State<AppState>, Query(pairs): Query<Vec<(String, String)>>) -> Response {
    let filters = SearchFilters::from_pairs(&pairs);
    let Ok(page) = catalog::search(&state.pool, &filters).await else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let Ok(destinations) = catalog::destinations(&state.pool).await else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let queried = SearchFilters::has_query_string(&pairs);
    let meta = meta(
        &state,
        "Find a hotel — Elsewhere",
        "Search independent hotels by destination, price, rating, amenities, and offers.",
        "/search",
        if queried { "noindex,follow" } else { "index,follow" },
        false,
        false,
        String::new(),
    );
    let search_props = search_form_props(&destinations, filters.destination.as_deref(), filters.query.as_deref(), filters.check_in.as_deref(), filters.check_out.as_deref(), filters.adults, filters.children, true);
    let filter_props = serde_json::to_string(&json!({
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
        "amenities": page.amenities.iter().map(|item| json!({"id": item.id, "name": item.name})).collect::<Vec<_>>(),
    })).unwrap_or_else(|_| "{}".into());
    let mut hidden = Vec::new();
    for (key, value) in &pairs {
        if key != "sort" && key != "page" {
            hidden.push(HiddenField { name: key.clone(), value: value.clone() });
        }
    }
    let page_links = (1..=page.pages).map(|number| PageLink {
        number,
        current: number == filters.page,
        href: page_href(&pairs, number),
    }).collect();
    render(
        SearchTemplate {
            meta: &meta,
            search_props,
            filter_props,
            total: page.total,
            sort: filters.sort.clone(),
            hidden,
            cards: page.results.iter().map(|hit| catalog::hotel_card(&hit.hotel, &state.images)).collect(),
            pages: page.pages,
            page_links,
        },
        if queried { "private, no-store" } else { "private, no-store" },
        StatusCode::OK,
    )
}

pub async fn inquire(State(state): State<AppState>, Query(pairs): Query<Vec<(String, String)>>) -> Response {
    let value = |key: &str| pairs.iter().find(|(name, _)| name == key).map(|(_, value)| value.clone()).filter(|value| !value.is_empty());
    let Some(hotel_id) = value("hotel") else {
        return missing(&state);
    };
    let room_id = value("room");
    let offer_id = value("offer");
    let Ok(found) = catalog::inquiry_context(&state.pool, &hotel_id, room_id.as_deref(), offer_id.as_deref()).await else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let Some((hotel, room, offer)) = found else {
        return missing(&state);
    };
    let meta = meta(&state, "Make an inquiry — Elsewhere", &format!("Ask about a stay at {}. No reservation or payment is made.", hotel.name), "/inquire", "noindex,nofollow", false, false, String::new());
    let inquiry_props = serde_json::to_string(&json!({
        "hotel": {"id": hotel.id, "name": hotel.name, "slug": hotel.slug},
        "room": room.as_ref().map(|room| json!({"id": room.id, "name": room.name})),
        "offer": offer.as_ref().map(|offer| json!({"id": offer.id, "title": offer.title})),
    })).unwrap_or_else(|_| "{}".into());
    render(
        InquireTemplate {
            meta: &meta,
            hero_image: hotel.hero_image,
            hotel_name: hotel.name,
            address: hotel.address,
            room_name: room.as_ref().map(|room| room.name.clone()).unwrap_or_default(),
            offer_title: offer.as_ref().map(|offer| offer.title.clone()).unwrap_or_default(),
            inquiry_props,
        },
        "private, no-store",
        StatusCode::OK,
    )
}

pub async fn admin_page(State(state): State<AppState>, jar: CookieJar, uri: axum::http::Uri) -> Response {
    let headers = axum::http::HeaderMap::new();
    let _ = headers;
    let cookie = jar.get("hotel-admin").map(|cookie| format!("hotel-admin={}", cookie.value()));
    let signed_in = cookie.as_deref().and_then(|value| auth::read_session(&state.config, value)).is_some();
    let path = uri.path();
    if path != "/admin/login" && !signed_in {
        return Redirect::to("/admin/login").into_response();
    }
    if path == "/admin/login" && signed_in {
        return Redirect::to("/admin").into_response();
    }
    let meta = meta(&state, "Admin studio — Elsewhere", "", path, "noindex,nofollow", false, true, String::new());
    render(AdminTemplate { meta: &meta }, "private, no-store", StatusCode::OK)
}

fn post_card(post: BlogPost) -> PostCard {
    PostCard { slug: post.slug, title: post.title, excerpt: post.excerpt, hero_image: post.hero_image }
}

fn search_form_props(destinations: &[Destination], destination: Option<&str>, query: Option<&str>, check_in: Option<&str>, check_out: Option<&str>, adults: i32, children: i32, compact: bool) -> String {
    serde_json::to_string(&json!({
        "destinations": destinations.iter().map(|item| json!({"slug": item.slug, "name": item.name})).collect::<Vec<_>>(),
        "initial": {"destination": destination, "query": query, "checkIn": check_in, "checkOut": check_out, "adults": adults, "children": children},
        "compact": compact,
    })).unwrap_or_else(|_| "{}".into())
}

fn page_href(pairs: &[(String, String)], page: i32) -> String {
    let mut query = Vec::new();
    for (key, value) in pairs {
        if key != "page" {
            query.push(format!("{}={}", catalog::urlencoding_escape(key), catalog::urlencoding_escape(value)));
        }
    }
    query.push(format!("page={page}"));
    format!("/search?{}", query.join("&"))
}

fn gallery_item(item: &GalleryImage, images: &ImageIndex) -> serde_json::Value {
    let variant = images.variants.get(&item.src);
    json!({
        "id": item.id,
        "src": variant.map(|image| image.src.as_str()).unwrap_or(item.src.as_str()),
        "alt": item.alt,
        "caption": item.caption,
        "category": item.category,
        "roomId": item.room_id,
        "webp": variant.map(|image| image.webp.as_str()).unwrap_or(""),
        "avif": variant.map(|image| image.avif.as_str()).unwrap_or(""),
    })
}

fn pretty_date(value: &str) -> String {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map(|date| date.format("%B %-d, %Y").to_string())
        .unwrap_or_else(|_| value.to_string())
}

#[allow(dead_code)]
fn _image_index(images: &ImageIndex) -> &ImageIndex {
    images
}

#[derive(Serialize)]
#[allow(dead_code)]
struct Empty {}
