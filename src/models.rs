use chrono::NaiveDateTime;
use serde::Serialize;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Destination {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub country: String,
    pub eyebrow: String,
    pub summary: String,
    pub content: serde_json::Value,
    pub hero_image: String,
    pub seo_title: String,
    pub seo_description: String,
    pub status: String,
    pub published_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hotel {
    pub id: String,
    pub destination_id: String,
    pub name: String,
    pub slug: String,
    pub property_type: String,
    pub address: String,
    pub summary: String,
    pub description: String,
    pub hero_image: String,
    pub rating: f64,
    pub star_rating: Option<i32>,
    pub review_count: i32,
    pub price_from: i32,
    pub currency: String,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub seo_title: String,
    pub seo_description: String,
    pub status: String,
    pub published_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Room {
    pub id: String,
    pub hotel_id: String,
    pub name: String,
    pub slug: String,
    pub summary: String,
    pub image: String,
    pub price_from: i32,
    pub max_guests: i32,
    pub size_sqm: Option<i32>,
    pub bed: String,
    pub status: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Offer {
    pub id: String,
    pub hotel_id: String,
    pub title: String,
    pub slug: String,
    pub summary: String,
    pub image: String,
    pub discount_percent: Option<i32>,
    pub valid_from: Option<String>,
    pub valid_to: Option<String>,
    pub terms: String,
    pub status: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Amenity {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub applies_to: String,
    pub category: String,
}

#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GalleryImage {
    pub id: String,
    pub hotel_id: String,
    pub room_id: Option<String>,
    pub src: String,
    pub alt: String,
    pub caption: Option<String>,
    pub category: String,
    pub sort_order: i32,
}

#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Highlight {
    pub id: String,
    pub hotel_id: String,
    pub title: String,
    pub summary: String,
    pub icon: String,
    pub sort_order: i32,
}

#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Fact {
    pub id: String,
    pub hotel_id: String,
    #[sqlx(rename = "group")]
    pub group_name: String,
    pub label: String,
    pub value: String,
    pub sort_order: i32,
}

#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Faq {
    pub id: String,
    pub hotel_id: String,
    pub question: String,
    pub answer: String,
    pub sort_order: i32,
}

#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NearbyPlace {
    pub id: String,
    pub hotel_id: String,
    pub name: String,
    pub category: String,
    pub distance_meters: i32,
    pub sort_order: i32,
}

#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Policy {
    pub id: String,
    pub hotel_id: String,
    pub category: String,
    pub title: String,
    pub description: String,
    pub sort_order: i32,
}

#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewScore {
    pub hotel_id: String,
    pub category: String,
    pub label: String,
    pub score: f64,
    pub sort_order: i32,
}

#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Review {
    pub id: String,
    pub hotel_id: String,
    pub room_id: Option<String>,
    pub guest_name: String,
    pub guest_country: String,
    pub traveler_type: String,
    pub rating: f64,
    pub title: String,
    pub body: String,
    pub stayed_at: String,
    pub reviewed_at: String,
    pub nights: i32,
    pub response: Option<String>,
    pub sort_order: i32,
}

#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlogPost {
    pub id: String,
    pub title: String,
    pub slug: String,
    pub excerpt: String,
    pub author: String,
    pub hero_image: String,
    pub content: serde_json::Value,
    pub seo_title: String,
    pub seo_description: String,
    pub status: String,
    pub published_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminUser {
    pub id: String,
    pub email: String,
    pub password_hash: String,
    pub name: String,
}
