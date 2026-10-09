# Elsewhere

An Axum hotel catalog with Askama for public HTML, Elm islands for interactive controls, and a protected Elm admin. PostgreSQL stores the catalog. A snapshot worker connected to Lightpanda publishes public HTML snapshots; visitors do not wait for browser rendering. Booking is an inquiry, not a reservation or payment.

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
docker compose up -d postgres lightpanda
cd frontend && npm install && npm run build && cd ..
cargo run --bin seed
SNAPSHOT_MODE=off cargo run
```

Open http://localhost:5000. The seeded admin is `admin@example.com` / `ChangeMe123!` in development when those variables are unset. `.env.example` sets the same pair.

For Vite during development, run `npm run dev` in `frontend` and start the server with `ASSET_MODE=vite`.

## Snapshots

Set `SNAPSHOT_MODE=on` and a `SNAPSHOT_INTERNAL_TOKEN` of at least 32 characters. When running Lightpanda in Docker, set `SNAPSHOT_API_ORIGIN=http://host.docker.internal:5000` (or leave default for local binary) so the browser can reach the host application.

Start Postgres and Lightpanda with Compose:

```sh
docker compose up -d postgres lightpanda
```

Install snapshot-worker dependencies (uses `playwright-core` over CDP; no Chromium browser download needed):

```sh
cd snapshot-worker && npm install && cd ..
node snapshot-worker/worker.mjs --once
```

### Lightpanda vs Playwright Docker Benchmark

Benchmark measured on all 58 canonical pages in the Elsewhere catalog under identical hardware and network conditions:

| Metric | Lightpanda Browser (Zig) | Playwright Chromium (Docker) | Improvement |
| --- | --- | --- | --- |
| **Full Snapshot Pass (58 pages)** | **5.22 s** (90.0 ms/page) | **24.53 s** (422.9 ms/page) | **4.7x faster** |
| **Idle Memory (Baseline)** | **12.16 MiB** | **143.20 MiB** | **11.8x lower** |
| **Peak Memory (58 pages)** | **16.19 MiB** (delta: +4.03 MiB) | **235.00 MiB** (delta: +91.80 MiB) | **14.5x lower** |
| **CPU Usage (Avg / Peak)** | **21.2% / 29.4%** | **74.1% / 86.2%** | **3.5x lower CPU load** |
| **Docker Compressed Image Size** | **58.3 MB** | **890.0 MB** | **15.3x smaller download** |
| **Docker Unpacked On-Disk Size** | **240.5 MB** (240,505,496 B) | **3.53 GB** (3,531,750,183 B) | **14.7x smaller disk size** |

The benchmark script can be reproduced via:
```sh
python3 scripts/benchmark.py
```

### CI Pipeline Speedup

With Lightpanda in CI (`.github/workflows/ci.yml`):
- **No browser/apt download step**: Pulling `lightpanda/browser:nightly` (58.3 MB) takes ~3–5s vs downloading full Chromium and installing Linux OS libraries (`npx playwright install --with-deps chromium`, ~45–80s).
- **Snapshot step speedup**: 58-page snapshot capture takes ~5s instead of ~25s.
- **Total estimated CI speedup**: ~60 to 95 seconds saved per CI run.

The worker leases `snapshot_jobs`, opens `/_snapshot-source` with the internal token, waits for `window.__SNAPSHOT_READY__`, and stores the HTML in `page_snapshots`. Canonical pages without a snapshot return 503 until the first capture. `/search` with a query string, `/inquire`, and `/admin` are never snapshotted.

## Checks

```sh
TEST_DATABASE_URL=postgresql://hotel:hotel_local_only@localhost:5432/hotel_test cargo test -- --test-threads=1
```
