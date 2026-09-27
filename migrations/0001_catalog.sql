CREATE TABLE admin_users (
    id text PRIMARY KEY,
    email text NOT NULL,
    password_hash text NOT NULL,
    name text NOT NULL,
    created_at timestamp without time zone NOT NULL DEFAULT now(),
    updated_at timestamp without time zone NOT NULL DEFAULT now(),
    CONSTRAINT admin_users_email_unique UNIQUE (email)
);

CREATE TABLE amenities (
    id text PRIMARY KEY,
    name text NOT NULL,
    icon text NOT NULL DEFAULT 'sparkles',
    applies_to text NOT NULL DEFAULT 'both',
    category text NOT NULL DEFAULT 'services',
    CONSTRAINT amenities_name_unique UNIQUE (name)
);

CREATE TABLE blog_posts (
    id text PRIMARY KEY,
    title text NOT NULL,
    slug text NOT NULL,
    excerpt text NOT NULL,
    author text NOT NULL,
    hero_image text NOT NULL,
    content jsonb NOT NULL,
    seo_title text NOT NULL,
    seo_description text NOT NULL,
    status text NOT NULL DEFAULT 'draft',
    published_at timestamp without time zone,
    created_at timestamp without time zone NOT NULL DEFAULT now(),
    updated_at timestamp without time zone NOT NULL DEFAULT now()
);

CREATE TABLE destinations (
    id text PRIMARY KEY,
    name text NOT NULL,
    slug text NOT NULL,
    country text NOT NULL,
    eyebrow text NOT NULL DEFAULT 'Destination',
    summary text NOT NULL,
    content jsonb NOT NULL,
    hero_image text NOT NULL,
    seo_title text NOT NULL,
    seo_description text NOT NULL,
    status text NOT NULL DEFAULT 'draft',
    published_at timestamp without time zone,
    created_at timestamp without time zone NOT NULL DEFAULT now(),
    updated_at timestamp without time zone NOT NULL DEFAULT now()
);

CREATE TABLE media_assets (
    id text PRIMARY KEY,
    filename text NOT NULL,
    mime_type text NOT NULL,
    width integer NOT NULL,
    height integer NOT NULL,
    alt text NOT NULL,
    caption text,
    focal_x double precision NOT NULL DEFAULT 0.5,
    focal_y double precision NOT NULL DEFAULT 0.5,
    variants jsonb NOT NULL,
    created_at timestamp without time zone NOT NULL DEFAULT now(),
    updated_at timestamp without time zone NOT NULL DEFAULT now()
);

CREATE TABLE page_snapshots (
    path text PRIMARY KEY,
    html text NOT NULL,
    asset_version text NOT NULL,
    generated_at timestamp without time zone NOT NULL
);

CREATE TABLE snapshot_jobs (
    path text PRIMARY KEY,
    desired_version text NOT NULL,
    attempts integer NOT NULL,
    next_attempt_at timestamp without time zone NOT NULL,
    leased_until timestamp without time zone
);

CREATE TABLE hotels (
    id text PRIMARY KEY,
    destination_id text NOT NULL REFERENCES destinations (id),
    name text NOT NULL,
    slug text NOT NULL,
    property_type text NOT NULL DEFAULT 'Boutique hotel',
    address text NOT NULL,
    summary text NOT NULL,
    description text NOT NULL,
    hero_image text NOT NULL,
    rating double precision NOT NULL,
    star_rating integer,
    review_count integer NOT NULL DEFAULT 0,
    price_from integer NOT NULL,
    currency text NOT NULL DEFAULT 'USD',
    latitude double precision,
    longitude double precision,
    seo_title text NOT NULL,
    seo_description text NOT NULL,
    status text NOT NULL DEFAULT 'draft',
    published_at timestamp without time zone,
    created_at timestamp without time zone NOT NULL DEFAULT now(),
    updated_at timestamp without time zone NOT NULL DEFAULT now()
);

CREATE TABLE hotel_amenities (
    hotel_id text NOT NULL REFERENCES hotels (id) ON DELETE CASCADE,
    amenity_id text NOT NULL REFERENCES amenities (id) ON DELETE CASCADE,
    PRIMARY KEY (hotel_id, amenity_id)
);

CREATE TABLE hotel_facts (
    id text PRIMARY KEY,
    hotel_id text NOT NULL REFERENCES hotels (id) ON DELETE CASCADE,
    "group" text NOT NULL,
    label text NOT NULL,
    value text NOT NULL,
    sort_order integer NOT NULL DEFAULT 0
);

CREATE TABLE hotel_faqs (
    id text PRIMARY KEY,
    hotel_id text NOT NULL REFERENCES hotels (id) ON DELETE CASCADE,
    question text NOT NULL,
    answer text NOT NULL,
    sort_order integer NOT NULL DEFAULT 0
);

CREATE TABLE hotel_highlights (
    id text PRIMARY KEY,
    hotel_id text NOT NULL REFERENCES hotels (id) ON DELETE CASCADE,
    title text NOT NULL,
    summary text NOT NULL,
    icon text NOT NULL,
    sort_order integer NOT NULL DEFAULT 0
);

CREATE TABLE hotel_nearby_places (
    id text PRIMARY KEY,
    hotel_id text NOT NULL REFERENCES hotels (id) ON DELETE CASCADE,
    name text NOT NULL,
    category text NOT NULL,
    distance_meters integer NOT NULL,
    sort_order integer NOT NULL DEFAULT 0
);

CREATE TABLE hotel_policies (
    id text PRIMARY KEY,
    hotel_id text NOT NULL REFERENCES hotels (id) ON DELETE CASCADE,
    category text NOT NULL,
    title text NOT NULL,
    description text NOT NULL,
    sort_order integer NOT NULL DEFAULT 0
);

CREATE TABLE hotel_review_scores (
    hotel_id text NOT NULL REFERENCES hotels (id) ON DELETE CASCADE,
    category text NOT NULL,
    label text NOT NULL,
    score double precision NOT NULL,
    sort_order integer NOT NULL DEFAULT 0,
    PRIMARY KEY (hotel_id, category)
);

CREATE TABLE offers (
    id text PRIMARY KEY,
    hotel_id text NOT NULL REFERENCES hotels (id) ON DELETE CASCADE,
    title text NOT NULL,
    slug text NOT NULL,
    summary text NOT NULL,
    image text NOT NULL,
    discount_percent integer,
    valid_from text,
    valid_to text,
    terms text NOT NULL,
    status text NOT NULL DEFAULT 'draft',
    created_at timestamp without time zone NOT NULL DEFAULT now(),
    updated_at timestamp without time zone NOT NULL DEFAULT now()
);

CREATE TABLE rooms (
    id text PRIMARY KEY,
    hotel_id text NOT NULL REFERENCES hotels (id) ON DELETE CASCADE,
    name text NOT NULL,
    slug text NOT NULL,
    summary text NOT NULL,
    image text NOT NULL,
    price_from integer NOT NULL,
    max_guests integer NOT NULL,
    size_sqm integer,
    bed text NOT NULL,
    status text NOT NULL DEFAULT 'draft',
    created_at timestamp without time zone NOT NULL DEFAULT now(),
    updated_at timestamp without time zone NOT NULL DEFAULT now()
);

CREATE TABLE hotel_gallery_images (
    id text PRIMARY KEY,
    hotel_id text NOT NULL REFERENCES hotels (id) ON DELETE CASCADE,
    room_id text REFERENCES rooms (id) ON DELETE SET NULL,
    src text NOT NULL,
    alt text NOT NULL,
    caption text,
    category text NOT NULL,
    sort_order integer NOT NULL DEFAULT 0
);

CREATE TABLE hotel_reviews (
    id text PRIMARY KEY,
    hotel_id text NOT NULL REFERENCES hotels (id) ON DELETE CASCADE,
    room_id text REFERENCES rooms (id) ON DELETE SET NULL,
    guest_name text NOT NULL,
    guest_country text NOT NULL,
    traveler_type text NOT NULL,
    rating double precision NOT NULL,
    title text NOT NULL,
    body text NOT NULL,
    stayed_at text NOT NULL,
    reviewed_at text NOT NULL,
    nights integer NOT NULL,
    response text,
    sort_order integer NOT NULL DEFAULT 0
);

CREATE TABLE inquiries (
    id text PRIMARY KEY,
    hotel_id text NOT NULL REFERENCES hotels (id),
    room_id text REFERENCES rooms (id),
    offer_id text REFERENCES offers (id),
    check_in text NOT NULL,
    check_out text NOT NULL,
    adults integer NOT NULL,
    children integer NOT NULL DEFAULT 0,
    name text NOT NULL,
    email text NOT NULL,
    phone text,
    message text,
    status text NOT NULL DEFAULT 'new',
    created_at timestamp without time zone NOT NULL DEFAULT now(),
    updated_at timestamp without time zone NOT NULL DEFAULT now()
);

CREATE TABLE room_amenities (
    room_id text NOT NULL REFERENCES rooms (id) ON DELETE CASCADE,
    amenity_id text NOT NULL REFERENCES amenities (id) ON DELETE CASCADE,
    PRIMARY KEY (room_id, amenity_id)
);

CREATE UNIQUE INDEX blog_posts_slug_idx ON blog_posts (slug);
CREATE UNIQUE INDEX destinations_slug_idx ON destinations (slug);
CREATE INDEX hotel_amenities_amenity_id_idx ON hotel_amenities (amenity_id);
CREATE INDEX hotel_facts_hotel_sort_idx ON hotel_facts (hotel_id, sort_order);
CREATE INDEX hotel_faqs_hotel_sort_idx ON hotel_faqs (hotel_id, sort_order);
CREATE INDEX hotel_gallery_images_hotel_sort_idx ON hotel_gallery_images (hotel_id, sort_order);
CREATE INDEX hotel_gallery_images_room_id_idx ON hotel_gallery_images (room_id);
CREATE INDEX hotel_highlights_hotel_sort_idx ON hotel_highlights (hotel_id, sort_order);
CREATE INDEX hotel_nearby_places_hotel_sort_idx ON hotel_nearby_places (hotel_id, sort_order);
CREATE INDEX hotel_policies_hotel_sort_idx ON hotel_policies (hotel_id, sort_order);
CREATE INDEX hotel_review_scores_hotel_sort_idx ON hotel_review_scores (hotel_id, sort_order);
CREATE INDEX hotel_reviews_hotel_sort_idx ON hotel_reviews (hotel_id, sort_order);
CREATE INDEX hotel_reviews_room_id_idx ON hotel_reviews (room_id);
CREATE INDEX hotels_destination_idx ON hotels (destination_id);
CREATE INDEX hotels_search_idx ON hotels (status, price_from, rating);
CREATE UNIQUE INDEX hotels_slug_idx ON hotels (slug);
CREATE INDEX inquiries_status_idx ON inquiries (status, created_at);
CREATE INDEX inquiries_hotel_id_idx ON inquiries (hotel_id);
CREATE INDEX inquiries_offer_id_idx ON inquiries (offer_id);
CREATE INDEX inquiries_room_id_idx ON inquiries (room_id);
CREATE UNIQUE INDEX offers_hotel_slug_idx ON offers (hotel_id, slug);
CREATE INDEX room_amenities_amenity_id_idx ON room_amenities (amenity_id);
CREATE UNIQUE INDEX rooms_hotel_slug_idx ON rooms (hotel_id, slug);
