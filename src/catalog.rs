use std::collections::HashMap;

use chrono::NaiveDate;
use serde::Serialize;
use serde_json::Value;
use sqlx::{PgPool, QueryBuilder, Row};

use crate::models::*;
use crate::util::{escape, price, rating};

#[derive(Debug, Clone)]
pub struct SearchFilters {
    pub destination: Option<String>,
    pub query: Option<String>,
    pub check_in: Option<String>,
    pub check_out: Option<String>,
    pub adults: i32,
    pub children: i32,
    pub min_price: Option<f64>,
    pub max_price: Option<f64>,
    pub rating: Option<f64>,
    pub amenities: Vec<String>,
    pub offers: bool,
    pub sort: String,
    pub page: i32,
}

impl SearchFilters {
    pub fn from_pairs(pairs: &[(String, String)]) -> Self {
        let mut values: HashMap<String, Vec<String>> = HashMap::new();
        for (key, value) in pairs {
            values.entry(key.clone()).or_default().push(value.clone());
        }
        let first = |key: &str| values.get(key).and_then(|items| items.first()).cloned();
        let bounded = |key: &str, max: usize| {
            first(key).map(|value| value.trim().to_string()).filter(|value| value.len() <= max && !value.is_empty())
        };
        let number = |key: &str, min: f64, max: f64| {
            first(key).and_then(|value| value.parse::<f64>().ok()).filter(|number| number.is_finite() && *number >= min && *number <= max)
        };
        let integer = |key: &str, fallback: i32, min: i32, max: i32| {
            first(key).and_then(|value| value.parse::<i32>().ok()).filter(|number| *number >= min && *number <= max).unwrap_or(fallback)
        };
        let date = |key: &str| {
            first(key).and_then(|value| NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d").ok()).map(|date| date.format("%Y-%m-%d").to_string())
        };
        let mut amenities = Vec::new();
        if let Some(items) = values.get("amenities") {
            for item in items {
                for part in item.split(',') {
                    let part = part.trim();
                    if !part.is_empty() && !amenities.iter().any(|existing: &String| existing == part) {
                        amenities.push(part.to_string());
                    }
                }
            }
        }
        let mut sort = first("sort").unwrap_or_else(|| "relevance".into());
        if !matches!(sort.as_str(), "price-asc" | "price-desc" | "rating") {
            sort = "relevance".into();
        }
        Self {
            destination: bounded("destination", 80),
            query: bounded("query", 100),
            check_in: date("checkIn"),
            check_out: date("checkOut"),
            adults: integer("adults", 2, 1, 12),
            children: integer("children", 0, 0, 8),
            min_price: number("minPrice", 0.0, f64::MAX),
            max_price: number("maxPrice", 0.0, f64::MAX),
            rating: number("rating", 0.0, 5.0),
            amenities,
            offers: first("offers").is_some_and(|value| value.eq_ignore_ascii_case("true")),
            sort,
            page: integer("page", 1, 1, i32::MAX),
        }
    }

    pub fn has_query_string(pairs: &[(String, String)]) -> bool {
        pairs.iter().any(|(_, value)| !value.is_empty())
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeOffer {
    #[serde(flatten)]
    pub offer: Offer,
    pub hotel_slug: String,
}

#[derive(Debug)]
pub struct HotelDetail {
    pub hotel: Hotel,
    pub destination: Destination,
    pub rooms: Vec<Room>,
    pub offers: Vec<Offer>,
    pub amenities: Vec<Amenity>,
    pub room_amenities: Vec<RoomAmenity>,
    pub gallery: Vec<GalleryImage>,
    pub highlights: Vec<Highlight>,
    pub facts: Vec<Fact>,
    pub faqs: Vec<Faq>,
    pub nearby: Vec<NearbyPlace>,
    pub policies: Vec<Policy>,
    pub review_scores: Vec<ReviewScore>,
    pub reviews: Vec<Review>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct RoomAmenity {
    pub room_id: String,
    pub id: String,
    pub name: String,
}

#[derive(Debug)]
pub struct SearchHit {
    pub hotel: Hotel,
    pub destination: Destination,
}

#[derive(Debug)]
pub struct SearchPage {
    pub results: Vec<SearchHit>,
    pub amenities: Vec<Amenity>,
    pub total: i64,
    pub pages: i32,
}

pub async fn destinations(pool: &PgPool) -> Result<Vec<Destination>, sqlx::Error> {
    sqlx::query_as("SELECT * FROM destinations WHERE status = 'published' ORDER BY name")
        .fetch_all(pool)
        .await
}

pub async fn posts(pool: &PgPool) -> Result<Vec<BlogPost>, sqlx::Error> {
    sqlx::query_as("SELECT * FROM blog_posts WHERE status = 'published' ORDER BY published_at DESC NULLS LAST, id")
        .fetch_all(pool)
        .await
}

pub async fn home(pool: &PgPool) -> Result<(Vec<Destination>, Vec<Hotel>, Vec<(Offer, String)>, Vec<BlogPost>), sqlx::Error> {
    let destinations = sqlx::query_as::<_, Destination>(
        "SELECT * FROM destinations WHERE status = 'published' ORDER BY name LIMIT 6",
    )
    .fetch_all(pool)
    .await?;
    let hotels = sqlx::query_as::<_, Hotel>(
        "SELECT h.* FROM hotels h JOIN destinations d ON d.id = h.destination_id
         WHERE h.status = 'published' AND d.status = 'published'
         ORDER BY h.rating DESC, h.id LIMIT 6",
    )
    .fetch_all(pool)
    .await?;
    let offers = home_offers(pool).await?;
    let posts = sqlx::query_as::<_, BlogPost>(
        "SELECT * FROM blog_posts WHERE status = 'published' ORDER BY published_at DESC NULLS LAST, id LIMIT 3",
    )
    .fetch_all(pool)
    .await?;
    Ok((destinations, hotels, offers, posts))
}

pub async fn home_offers(pool: &PgPool) -> Result<Vec<(Offer, String)>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT o.id, o.hotel_id, o.title, o.slug AS offer_slug, o.summary, o.image,
                o.discount_percent, o.valid_from, o.valid_to, o.terms, o.status,
                o.created_at, o.updated_at, h.slug AS hotel_slug
         FROM offers o
         JOIN hotels h ON h.id = o.hotel_id
         JOIN destinations d ON d.id = h.destination_id
         WHERE o.status = 'published' AND h.status = 'published' AND d.status = 'published'
         ORDER BY o.id LIMIT 3",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            (
                Offer {
                    id: row.get("id"),
                    hotel_id: row.get("hotel_id"),
                    title: row.get("title"),
                    slug: row.get("offer_slug"),
                    summary: row.get("summary"),
                    image: row.get("image"),
                    discount_percent: row.get("discount_percent"),
                    valid_from: row.get("valid_from"),
                    valid_to: row.get("valid_to"),
                    terms: row.get("terms"),
                    status: row.get("status"),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                },
                row.get("hotel_slug"),
            )
        })
        .collect())
}

pub async fn destination(pool: &PgPool, slug: &str) -> Result<Option<(Destination, Vec<Hotel>)>, sqlx::Error> {
    let destination = sqlx::query_as::<_, Destination>(
        "SELECT * FROM destinations WHERE slug = $1 AND status = 'published'",
    )
    .bind(slug)
    .fetch_optional(pool)
    .await?;
    let Some(destination) = destination else {
        return Ok(None);
    };
    let hotels = sqlx::query_as::<_, Hotel>(
        "SELECT * FROM hotels WHERE destination_id = $1 AND status = 'published' ORDER BY rating DESC, id",
    )
    .bind(&destination.id)
    .fetch_all(pool)
    .await?;
    Ok(Some((destination, hotels)))
}

pub async fn hotel(pool: &PgPool, slug: &str) -> Result<Option<HotelDetail>, sqlx::Error> {
    let hotel = sqlx::query_as::<_, Hotel>(
        "SELECT h.* FROM hotels h JOIN destinations d ON d.id = h.destination_id
         WHERE h.slug = $1 AND h.status = 'published' AND d.status = 'published'",
    )
    .bind(slug)
    .fetch_optional(pool)
    .await?;
    let Some(hotel) = hotel else {
        return Ok(None);
    };
    let destination = sqlx::query_as::<_, Destination>("SELECT * FROM destinations WHERE id = $1")
        .bind(&hotel.destination_id)
        .fetch_one(pool)
        .await?;
    let rooms = sqlx::query_as::<_, Room>(
        "SELECT * FROM rooms WHERE hotel_id = $1 AND status = 'published' ORDER BY price_from, id",
    )
    .bind(&hotel.id)
    .fetch_all(pool)
    .await?;
    let offers = sqlx::query_as::<_, Offer>(
        "SELECT * FROM offers WHERE hotel_id = $1 AND status = 'published' ORDER BY id",
    )
    .bind(&hotel.id)
    .fetch_all(pool)
    .await?;
    let amenities = sqlx::query_as::<_, Amenity>(
        "SELECT a.* FROM amenities a JOIN hotel_amenities ha ON ha.amenity_id = a.id
         WHERE ha.hotel_id = $1 ORDER BY a.category, a.name",
    )
    .bind(&hotel.id)
    .fetch_all(pool)
    .await?;
    let room_amenities = sqlx::query_as::<_, RoomAmenity>(
        "SELECT ra.room_id, a.id, a.name FROM room_amenities ra
         JOIN amenities a ON a.id = ra.amenity_id
         JOIN rooms r ON r.id = ra.room_id
         WHERE r.hotel_id = $1 ORDER BY a.name",
    )
    .bind(&hotel.id)
    .fetch_all(pool)
    .await?;
    let gallery = sqlx::query_as::<_, GalleryImage>(
        "SELECT * FROM hotel_gallery_images WHERE hotel_id = $1 ORDER BY sort_order, id",
    )
    .bind(&hotel.id)
    .fetch_all(pool)
    .await?;
    let highlights = sqlx::query_as::<_, Highlight>(
        "SELECT * FROM hotel_highlights WHERE hotel_id = $1 ORDER BY sort_order, id",
    )
    .bind(&hotel.id)
    .fetch_all(pool)
    .await?;
    let facts = sqlx::query_as::<_, Fact>(
        "SELECT id, hotel_id, \"group\", label, value, sort_order FROM hotel_facts WHERE hotel_id = $1 ORDER BY sort_order, id",
    )
    .bind(&hotel.id)
    .fetch_all(pool)
    .await?;
    let faqs = sqlx::query_as::<_, Faq>(
        "SELECT * FROM hotel_faqs WHERE hotel_id = $1 ORDER BY sort_order, id",
    )
    .bind(&hotel.id)
    .fetch_all(pool)
    .await?;
    let nearby = sqlx::query_as::<_, NearbyPlace>(
        "SELECT * FROM hotel_nearby_places WHERE hotel_id = $1 ORDER BY sort_order, id",
    )
    .bind(&hotel.id)
    .fetch_all(pool)
    .await?;
    let policies = sqlx::query_as::<_, Policy>(
        "SELECT * FROM hotel_policies WHERE hotel_id = $1 ORDER BY sort_order, id",
    )
    .bind(&hotel.id)
    .fetch_all(pool)
    .await?;
    let review_scores = sqlx::query_as::<_, ReviewScore>(
        "SELECT * FROM hotel_review_scores WHERE hotel_id = $1 ORDER BY sort_order, category",
    )
    .bind(&hotel.id)
    .fetch_all(pool)
    .await?;
    let reviews = sqlx::query_as::<_, Review>(
        "SELECT * FROM hotel_reviews WHERE hotel_id = $1 ORDER BY sort_order, id",
    )
    .bind(&hotel.id)
    .fetch_all(pool)
    .await?;
    Ok(Some(HotelDetail {
        hotel,
        destination,
        rooms,
        offers,
        amenities,
        room_amenities,
        gallery,
        highlights,
        facts,
        faqs,
        nearby,
        policies,
        review_scores,
        reviews,
    }))
}

pub async fn offer(pool: &PgPool, slug: &str, offer_slug: &str) -> Result<Option<(Offer, Hotel, Destination)>, sqlx::Error> {
    let hotel = sqlx::query_as::<_, Hotel>(
        "SELECT h.* FROM hotels h JOIN destinations d ON d.id = h.destination_id
         WHERE h.slug = $1 AND h.status = 'published' AND d.status = 'published'",
    )
    .bind(slug)
    .fetch_optional(pool)
    .await?;
    let Some(hotel) = hotel else {
        return Ok(None);
    };
    let offer = sqlx::query_as::<_, Offer>(
        "SELECT * FROM offers WHERE hotel_id = $1 AND slug = $2 AND status = 'published'",
    )
    .bind(&hotel.id)
    .bind(offer_slug)
    .fetch_optional(pool)
    .await?;
    let Some(offer) = offer else {
        return Ok(None);
    };
    let destination = sqlx::query_as::<_, Destination>("SELECT * FROM destinations WHERE id = $1")
        .bind(&hotel.destination_id)
        .fetch_one(pool)
        .await?;
    Ok(Some((offer, hotel, destination)))
}

pub async fn post(pool: &PgPool, slug: &str) -> Result<Option<(BlogPost, HashMap<String, Hotel>)>, sqlx::Error> {
    let post = sqlx::query_as::<_, BlogPost>(
        "SELECT * FROM blog_posts WHERE slug = $1 AND status = 'published'",
    )
    .bind(slug)
    .fetch_optional(pool)
    .await?;
    let Some(post) = post else {
        return Ok(None);
    };
    let hotels = sqlx::query_as::<_, Hotel>(
        "SELECT h.* FROM hotels h JOIN destinations d ON d.id = h.destination_id
         WHERE h.status = 'published' AND d.status = 'published'",
    )
    .fetch_all(pool)
    .await?;
    let map = hotels.into_iter().map(|hotel| (hotel.id.clone(), hotel)).collect();
    Ok(Some((post, map)))
}

pub async fn inquiry_context(
    pool: &PgPool,
    hotel_id: &str,
    room_id: Option<&str>,
    offer_id: Option<&str>,
) -> Result<Option<(Hotel, Option<Room>, Option<Offer>)>, sqlx::Error> {
    let hotel = sqlx::query_as::<_, Hotel>(
        "SELECT h.* FROM hotels h JOIN destinations d ON d.id = h.destination_id
         WHERE h.id = $1 AND h.status = 'published' AND d.status = 'published'",
    )
    .bind(hotel_id)
    .fetch_optional(pool)
    .await?;
    let Some(hotel) = hotel else {
        return Ok(None);
    };
    let room = if let Some(room_id) = room_id.filter(|value| !value.is_empty()) {
        let room = sqlx::query_as::<_, Room>(
            "SELECT * FROM rooms WHERE id = $1 AND hotel_id = $2 AND status = 'published'",
        )
        .bind(room_id)
        .bind(&hotel.id)
        .fetch_optional(pool)
        .await?;
        if room.is_none() {
            return Ok(None);
        }
        room
    } else {
        None
    };
    let offer = if let Some(offer_id) = offer_id.filter(|value| !value.is_empty()) {
        let offer = sqlx::query_as::<_, Offer>(
            "SELECT * FROM offers WHERE id = $1 AND hotel_id = $2 AND status = 'published'",
        )
        .bind(offer_id)
        .bind(&hotel.id)
        .fetch_optional(pool)
        .await?;
        if offer.is_none() {
            return Ok(None);
        }
        offer
    } else {
        None
    };
    Ok(Some((hotel, room, offer)))
}

fn push_filters<'a>(builder: &mut QueryBuilder<'a, sqlx::Postgres>, filters: &'a SearchFilters) {
    if let Some(destination) = &filters.destination {
        builder.push(" AND d.slug = ");
        builder.push_bind(destination);
    }
    if let Some(query) = &filters.query {
        let pattern = format!(
            "%{}%",
            query.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
        );
        builder.push(" AND (h.name ILIKE ");
        builder.push_bind(pattern.clone());
        builder.push(" ESCAPE '\\' OR h.summary ILIKE ");
        builder.push_bind(pattern.clone());
        builder.push(" ESCAPE '\\' OR d.name ILIKE ");
        builder.push_bind(pattern);
        builder.push(" ESCAPE '\\')");
    }
    if let Some(min_price) = filters.min_price {
        builder.push(" AND h.price_from >= ");
        builder.push_bind(min_price);
    }
    if let Some(max_price) = filters.max_price {
        builder.push(" AND h.price_from <= ");
        builder.push_bind(max_price);
    }
    if let Some(rating) = filters.rating {
        builder.push(" AND h.rating >= ");
        builder.push_bind(rating);
    }
    if filters.offers {
        builder.push(" AND EXISTS (SELECT 1 FROM offers o WHERE o.hotel_id = h.id AND o.status = 'published')");
    }
    for amenity in &filters.amenities {
        builder.push(" AND EXISTS (SELECT 1 FROM hotel_amenities ha WHERE ha.hotel_id = h.id AND ha.amenity_id = ");
        builder.push_bind(amenity);
        builder.push(")");
    }
}

pub async fn search(pool: &PgPool, filters: &SearchFilters) -> Result<SearchPage, sqlx::Error> {
    let mut count = QueryBuilder::new(
        "SELECT COUNT(*) FROM hotels h JOIN destinations d ON d.id = h.destination_id
         WHERE h.status = 'published' AND d.status = 'published'",
    );
    push_filters(&mut count, filters);
    let total: i64 = count.build_query_scalar().fetch_one(pool).await?;
    let mut query = QueryBuilder::new(
        "SELECT h.id, h.destination_id, h.name, h.slug, h.property_type, h.address, h.summary,
                h.description, h.hero_image, h.rating, h.star_rating, h.review_count, h.price_from,
                h.currency, h.latitude, h.longitude, h.seo_title, h.seo_description, h.status,
                h.published_at, h.created_at, h.updated_at,
                d.id AS d_id, d.name AS d_name, d.slug AS d_slug, d.country AS d_country,
                d.eyebrow AS d_eyebrow, d.summary AS d_summary, d.content AS d_content,
                d.hero_image AS d_hero, d.seo_title AS d_seo_title, d.seo_description AS d_seo_description,
                d.status AS d_status, d.published_at AS d_published_at, d.created_at AS d_created_at,
                d.updated_at AS d_updated_at
         FROM hotels h JOIN destinations d ON d.id = h.destination_id
         WHERE h.status = 'published' AND d.status = 'published'",
    );
    push_filters(&mut query, filters);
    match filters.sort.as_str() {
        "price-asc" => query.push(" ORDER BY h.price_from, h.id"),
        "price-desc" => query.push(" ORDER BY h.price_from DESC, h.id"),
        "rating" => query.push(" ORDER BY h.rating DESC, h.id"),
        _ => query.push(" ORDER BY h.id"),
    };
    let offset = (filters.page as i64 - 1) * 9;
    let results = if offset >= total {
        Vec::new()
    } else {
        query.push(" LIMIT 9 OFFSET ");
        query.push_bind(offset);
        let rows = query.build().fetch_all(pool).await?;
        rows.into_iter()
            .map(|row| SearchHit {
                hotel: Hotel {
                    id: row.get("id"),
                    destination_id: row.get("destination_id"),
                    name: row.get("name"),
                    slug: row.get("slug"),
                    property_type: row.get("property_type"),
                    address: row.get("address"),
                    summary: row.get("summary"),
                    description: row.get("description"),
                    hero_image: row.get("hero_image"),
                    rating: row.get("rating"),
                    star_rating: row.get("star_rating"),
                    review_count: row.get("review_count"),
                    price_from: row.get("price_from"),
                    currency: row.get("currency"),
                    latitude: row.get("latitude"),
                    longitude: row.get("longitude"),
                    seo_title: row.get("seo_title"),
                    seo_description: row.get("seo_description"),
                    status: row.get("status"),
                    published_at: row.get("published_at"),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                },
                destination: Destination {
                    id: row.get("d_id"),
                    name: row.get("d_name"),
                    slug: row.get("d_slug"),
                    country: row.get("d_country"),
                    eyebrow: row.get("d_eyebrow"),
                    summary: row.get("d_summary"),
                    content: row.get("d_content"),
                    hero_image: row.get("d_hero"),
                    seo_title: row.get("d_seo_title"),
                    seo_description: row.get("d_seo_description"),
                    status: row.get("d_status"),
                    published_at: row.get("d_published_at"),
                    created_at: row.get("d_created_at"),
                    updated_at: row.get("d_updated_at"),
                },
            })
            .collect()
    };
    let amenities = sqlx::query_as::<_, Amenity>("SELECT * FROM amenities ORDER BY name")
        .fetch_all(pool)
        .await?;
    let pages = ((total as f64) / 9.0).ceil().max(1.0) as i32;
    Ok(SearchPage { results, amenities, total, pages })
}

pub fn hotel_card(hotel: &Hotel, images: &ImageIndex) -> String {
    let href = format!("/hotels/{}", urlencoding_escape(&hotel.slug));
    let picture = picture(&hotel.hero_image, &hotel.name, 560, 380, "(max-width: 700px) 100vw, (max-width: 1200px) 50vw, 33vw", images);
    format!(
        r#"<article class="card hotel-card"><a href="{href}" class="hotel-card__image-link"><span class="blur-image hotel-card__image-wrap">{picture}</span><span class="tag is-dark is-rounded hotel-card__rating">★ {rating}</span></a><div class="card-content hotel-card__body"><p class="hotel-card__location">Independent stay</p><h3 class="hotel-card__title"><a href="{href}">{name}</a></h3><p class="hotel-card__summary">{summary}</p></div><footer class="hotel-card__footer"><span class="hotel-card__price">From <strong>{price}</strong> / night</span><a href="{href}" class="button is-ghost button-icon" aria-label="Explore {name}">→</a></footer></article>"#,
        rating = rating(hotel.rating),
        name = escape(&hotel.name),
        summary = escape(&hotel.summary),
        price = escape(&price(hotel.price_from, &hotel.currency)),
    )
}

pub fn destination_card(destination: &Destination, images: &ImageIndex) -> String {
    let href = format!("/destinations/{}", urlencoding_escape(&destination.slug));
    let picture = picture(&destination.hero_image, &format!("{} landscape", destination.name), 560, 680, "(max-width: 700px) 100vw, (max-width: 1200px) 50vw, 33vw", images);
    format!(
        r#"<article class="card destination-card"><a class="destination-card__link" href="{href}"><span class="blur-image destination-card__image-wrap">{picture}</span><div class="destination-card__content"><span class="destination-card__country">{country}</span><h3>{name}</h3><p>{summary}</p><b>Explore →</b></div></a></article>"#,
        country = escape(&destination.country),
        name = escape(&destination.name),
        summary = escape(&destination.summary),
    )
}

pub fn offer_card(offer: &Offer, hotel_slug: &str, images: &ImageIndex) -> String {
    let href = format!("/hotels/{}/offers/{}", urlencoding_escape(hotel_slug), urlencoding_escape(&offer.slug));
    let label = offer.discount_percent.map(|value| format!("{value}% value")).unwrap_or_else(|| "Seasonal offer".into());
    let picture = picture(&offer.image, &offer.title, 240, 180, "240px", images);
    format!(
        r#"<article class="card offer-card"><span class="blur-image offer-card__image-wrap">{picture}</span><div class="card-content offer-card__body"><span class="tag is-secondary is-rounded">{label}</span><h3>{title}</h3><p>{summary}</p><a class="button is-text offer-card__link" href="{href}">View offer →</a></div></article>"#,
        label = escape(&label),
        title = escape(&offer.title),
        summary = escape(&offer.summary),
    )
}

#[derive(Clone)]
pub struct ImageVariant {
    pub src: String,
    pub avif: String,
    pub webp: String,
}

#[derive(Clone, Default)]
pub struct ImageIndex {
    pub variants: HashMap<String, ImageVariant>,
}

impl ImageIndex {
    pub fn load() -> Self {
        let Ok(text) = std::fs::read_to_string("static/images/image-manifest.json") else {
            return Self::default();
        };
        let Ok(value) = serde_json::from_str::<Value>(&text) else {
            return Self::default();
        };
        let Some(object) = value.as_object() else {
            return Self::default();
        };
        let mut variants = HashMap::new();
        for (key, item) in object {
            variants.insert(
                key.clone(),
                ImageVariant {
                    src: item.get("src").and_then(Value::as_str).unwrap_or("").to_string(),
                    avif: item.get("avif").and_then(Value::as_str).unwrap_or("").to_string(),
                    webp: item.get("webp").and_then(Value::as_str).unwrap_or("").to_string(),
                },
            );
        }
        Self { variants }
    }
}

pub fn picture(src: &str, alt: &str, width: u32, height: u32, sizes: &str, images: &ImageIndex) -> String {
    let variant = images.variants.get(src);
    let fallback = variant.map(|item| item.src.as_str()).unwrap_or(src);
    let sources = if let Some(variant) = variant {
        format!(
            r#"<source type="image/avif" srcset="{avif}" sizes="{sizes}" /><source type="image/webp" srcset="{webp}" sizes="{sizes}" />"#,
            avif = escape(&variant.avif),
            webp = escape(&variant.webp),
            sizes = escape(sizes),
        )
    } else {
        String::new()
    };
    format!(
        r#"<picture class="blur-image__picture">{sources}<img class="blur-image__image" src="{src}" alt="{alt}" width="{width}" height="{height}" loading="lazy" sizes="{sizes}" /></picture>"#,
        src = escape(fallback),
        alt = escape(alt),
        sizes = escape(sizes),
    )
}

pub fn urlencoding_escape(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

pub fn render_rich(content: &Value, hotels: Option<&HashMap<String, Hotel>>, images: &ImageIndex) -> String {
    let Some(nodes) = content.as_array() else {
        return String::new();
    };
    let mut html = String::from(r#"<div class="rich-content">"#);
    for node in nodes {
        let kind = node.get("type").and_then(Value::as_str).unwrap_or("");
        match kind {
            "heading" => {
                let level = node.get("level").and_then(Value::as_i64).unwrap_or(2);
                let tag = if level == 3 { "h3" } else if level == 4 { "h4" } else { "h2" };
                html.push_str(&format!("<{tag}>{}</{tag}>", escape(text(node, "text"))));
            }
            "paragraph" => html.push_str(&format!("<p>{}</p>", escape(text(node, "text")))),
            "blockquote" => html.push_str(&format!("<blockquote>{}</blockquote>", escape(text(node, "text")))),
            "bulletList" | "orderedList" => {
                let tag = if kind == "bulletList" { "ul" } else { "ol" };
                html.push_str(&format!("<{tag}>"));
                if let Some(items) = node.get("items").and_then(Value::as_array) {
                    for item in items {
                        let item_text = item.as_str().unwrap_or("");
                        html.push_str(&format!("<li>{}</li>", escape(item_text)));
                    }
                }
                html.push_str(&format!("</{tag}>"));
            }
            "image" => {
                if let Some(src) = safe_image(text(node, "src")) {
                    html.push_str(&format!(
                        r#"<figure><img src="{}" alt="{}" width="1280" height="720" loading="lazy" /></figure>"#,
                        escape(src),
                        escape(text(node, "alt"))
                    ));
                }
            }
            "table" => {
                if let Some(rows) = node.get("rows").and_then(Value::as_array) {
                    if !rows.is_empty() {
                        html.push_str(r#"<div class="table-container"><table class="table is-bordered"><thead><tr>"#);
                        if let Some(header) = rows[0].as_array() {
                            for cell in header {
                                html.push_str(&format!("<th scope=\"col\">{}</th>", escape(cell.as_str().unwrap_or(""))));
                            }
                        }
                        html.push_str("</tr></thead><tbody>");
                        for row in rows.iter().skip(1) {
                            html.push_str("<tr>");
                            if let Some(cells) = row.as_array() {
                                for cell in cells {
                                    html.push_str(&format!("<td>{}</td>", escape(cell.as_str().unwrap_or(""))));
                                }
                            }
                            html.push_str("</tr>");
                        }
                        html.push_str("</tbody></table></div>");
                    }
                }
            }
            "hotelEmbed" => {
                if let Some(hotels) = hotels {
                    if let Some(hotel) = hotels.get(text(node, "entityId")) {
                        html.push_str(r#"<aside class="rich-content__hotel"><span class="tag is-primary">Selected stay</span>"#);
                        html.push_str(&hotel_card(hotel, images));
                        html.push_str("</aside>");
                    }
                }
            }
            _ => {}
        }
    }
    html.push_str("</div>");
    html
}

fn text<'a>(node: &'a Value, key: &str) -> &'a str {
    node.get(key).and_then(Value::as_str).unwrap_or("")
}

fn safe_image(src: &str) -> Option<&str> {
    if src.starts_with('/') && !src.starts_with("//") && !src.contains('\\') {
        Some(src)
    } else if src.starts_with("https://") {
        Some(src)
    } else {
        None
    }
}
