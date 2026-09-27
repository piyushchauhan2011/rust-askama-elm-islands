# Elsewhere

An Axum hotel catalog with Askama for public HTML, Elm islands for interactive controls, and a protected Elm admin. PostgreSQL stores the catalog. A Playwright worker publishes public HTML snapshots; visitors do not wait for browser rendering. Booking is an inquiry, not a reservation or payment.

This is a port of the ASP.NET Razor and React version. The request ownership is the same:

| Surface | Implementation |
| --- | --- |
| Canonical public pages | Askama, then a PostgreSQL HTML snapshot when `SNAPSHOT_MODE=on` |
| `/search?...` | Askama results and an Elm filter island. Private, `noindex` |
| `/inquire` | Askama stay context and an Elm form. CSRF-protected |
| `/admin` | Askama document guard and an Elm admin application |
| `/api/*` | JSON for the catalog, search, inquiries, and admin |

Elm 0.19 does not hydrate existing markup. Askama emits empty `[data-island]` markers with `data-props`. The snapshot worker captures the page after Elm fills those markers. Visitors receive that HTML, and the island script initializes again from the same flags.

## Run it

Install Rust, Node, and Docker. From this directory:

```sh
cp .env.example .env
docker compose up -d postgres
cd frontend && npm install && npm run build && cd ..
cargo run --bin seed
SNAPSHOT_MODE=off cargo run
```

Open http://localhost:5000. The seeded admin is `admin@example.com` / `ChangeMe123!` in development when those variables are unset. `.env.example` sets the same pair.

For Vite during development, run `npm run dev` in `frontend` and start the server with `ASSET_MODE=vite`.

## Snapshots

Set `SNAPSHOT_MODE=on` and a `SNAPSHOT_INTERNAL_TOKEN` of at least 32 characters, then:

```sh
cd snapshot-worker && npm install && npx playwright install chromium && cd ..
node snapshot-worker/worker.mjs --once
```

The worker leases `snapshot_jobs`, opens `/_snapshot-source` with the internal token, waits for `window.__SNAPSHOT_READY__`, and stores the HTML in `page_snapshots`. Canonical pages without a snapshot return 503 until the first capture. `/search` with a query string, `/inquire`, and `/admin` are never snapshotted.

## Checks

```sh
TEST_DATABASE_URL=postgresql://hotel:hotel_local_only@localhost:5432/hotel_test cargo test -- --test-threads=1
```
