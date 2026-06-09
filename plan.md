# Strawtea Implementation Plan

## Summary

Build the first working Strawtea product as two apps:

- `strawtea-be`: Rust backend using Axum, SQLx, PostgreSQL, Supabase JWT verification, and a Twelve Data market-data adapter.
- `strawtea-ui`: Svelte + Vite SPA/PWA using Supabase Gmail login, client-side routing, a phone-menu home page, ticker search, and a one-month stock chart.
- Persist app-owned data in PostgreSQL with portable SQL migrations. Supabase-hosted Postgres is allowed only as plain Postgres.

## Implementation Changes

### Backend

- Initialize `strawtea-be` as a Rust binary crate.
- Use Axum for HTTP, Tokio for async runtime, SQLx for Postgres, Serde for JSON, Reqwest for outbound API calls, Tower HTTP for CORS/tracing, and JsonWebToken/JWKS-compatible crates for Supabase JWT validation.
- Add config loaded from environment:
  - `DATABASE_URL`
  - `SUPABASE_JWT_ISSUER`
  - `SUPABASE_JWT_AUDIENCE`
  - `SUPABASE_JWT_JWKS_URL`
  - `TWELVE_DATA_API_KEY`
  - `HTTP_ADDR`, default `127.0.0.1:8080`
- Implement endpoints:
  - `GET /healthz`: public health check.
  - `GET /api/me`: protected; verifies Supabase JWT, upserts internal user, returns user profile.
  - `GET /api/stocks/search?q=<query>`: protected; proxies ticker search through Twelve Data.
  - `GET /api/stocks/{ticker}/prices?range=1mo`: protected; returns normalized one-month price candles.
- Normalize stock API responses into:
  - `TickerSearchResult { symbol, name, exchange, asset_type }`
  - `PricePoint { date, open, high, low, close, volume }`
- Add portable SQLx migrations:
  - `users`: internal UUID id, Supabase user id, email, timestamps.
  - `stock_watchlist_items`: user id, ticker, timestamp, reserved for later product use.
- Keep Supabase database features out of migrations and backend logic.

### Frontend

- Initialize `strawtea-ui` as a Svelte + Vite + TypeScript SPA.
- Add PWA support with Vite PWA plugin, manifest, installable app metadata, and app-shell asset caching.
- Use Supabase JS client only for Gmail OAuth/session management.
- Use client-side routing with routes:
  - `/`: authenticated phone-menu home page.
  - `/stocks`: authenticated stocks page.
  - fallback route: not-found view inside the SPA.
- Add app state modules:
  - `auth`: Supabase session, login, logout, token access.
  - `api`: backend fetch wrapper that sends `Authorization: Bearer <token>`.
- Home page:
  - Full-screen mobile-first layout.
  - Large square icon grid.
  - Initial `Stocks` icon navigates to `/stocks`.
- Stocks page:
  - Search input with debounce.
  - Results list from `/api/stocks/search`.
  - Selected ticker state.
  - One-month chart loaded from `/api/stocks/{ticker}/prices?range=1mo`.
  - Use a Svelte-friendly chart library or lightweight canvas/SVG chart; keep backend response format provider-independent.
- Add environment variables:
  - `VITE_SUPABASE_URL`
  - `VITE_SUPABASE_ANON_KEY`
  - `VITE_API_BASE_URL`

### Development Setup

- Keep existing per-folder `flake.nix` and `.envrc`.
- Update flakes only if needed for missing project tooling:
  - Backend: Rust toolchain, SQLx CLI, cargo-watch, cargo-nextest, rust-analyzer.
  - Frontend: Node 24, Corepack, Svelte language server, TypeScript language server, Prettier.
- Add example env files:
  - `strawtea-be/.env.example`
  - `strawtea-ui/.env.example`
- Do not commit real secrets.

## Public Interfaces

- Backend API requires Supabase access token on all `/api/*` routes.
- `GET /api/me` response:
  - `id`
  - `email`
  - `created_at`
- `GET /api/stocks/search?q=AAPL` response:
  - array of `{ symbol, name, exchange, asset_type }`
- `GET /api/stocks/AAPL/prices?range=1mo` response:
  - `{ ticker, range: "1mo", prices: PricePoint[] }`
- Frontend never calls PostgreSQL or Twelve Data directly.

## Test Plan

- Backend:
  - `cargo fmt`
  - `cargo clippy --all-targets --all-features`
  - `cargo test`
  - SQLx migration check against a local or configured Postgres database.
  - Unit tests for config parsing, auth extraction failures, and market-data response normalization.
  - HTTP tests for `/healthz`, unauthorized `/api/me`, and protected route behavior with mocked auth.
- Frontend:
  - `npm run check`
  - `npm run build`
  - Component tests for home navigation, ticker search states, and chart empty/error/loading states.
  - Manual browser test for Supabase Gmail login redirect, `/` to `/stocks` navigation, ticker search, and chart rendering.
- PWA:
  - Verify manifest exists.
  - Verify production build registers service worker.
  - Verify app shell reloads when offline after first load.

## Assumptions

- Backend stack is Axum + SQLx.
- Market-data provider is Twelve Data for v1.
- Frontend stack is Svelte + Vite SPA, not SvelteKit.
- Supabase is used for Gmail authentication only; Supabase-hosted Postgres may be used as plain PostgreSQL.
- The initial product does not need a watchlist UI, even though the database can include a reserved watchlist table for later.
