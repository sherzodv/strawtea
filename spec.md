# Strawtea Architecture Spec

## Architecture Goals

- Keep the frontend as a Svelte SPA/PWA with fast client-side navigation.
- Keep business logic and trusted integrations in the Rust backend.
- Use Supabase for Gmail authentication, but avoid coupling application storage to Supabase-specific database features.
- Use PostgreSQL as the system of record in a way that can move from Supabase-hosted Postgres to self-hosted Postgres.
- Keep market-data APIs replaceable.

## System Overview

```text
Browser PWA
  |
  | Supabase OAuth login
  v
Supabase Auth
  |
  | JWT access token
  v
Svelte SPA  ---- authenticated API requests ---->  Rust Backend  ----> PostgreSQL
                                                        |
                                                        v
                                                  Market Data APIs
```

## Frontend

### Stack

- Svelte SPA built with Vite.
- TypeScript for application code.
- PWA support through a service worker and web app manifest.
- Client-side routing for all app pages.

### Responsibilities

- Render the app shell and route between feature pages without full reloads.
- Handle Supabase Gmail login and receive the Supabase session.
- Attach the Supabase access token to backend API requests.
- Render the phone-menu home page.
- Render the stocks search and chart experience.
- Cache static app assets for PWA installability and resilient loading.

### Suggested Structure

```text
src/
  app/
    routes.ts
    auth.ts
    api.ts
  features/
    home/
      HomePage.svelte
    stocks/
      StocksPage.svelte
      TickerSearch.svelte
      StockChart.svelte
  shared/
    components/
    stores/
    styles/
```

### Routing

- `/` renders the phone-menu home page.
- `/stocks` renders the stocks feature page.
- Unknown routes fall back to the SPA entry and show a client-side not-found view.

### Auth Flow

1. User opens the PWA.
2. Frontend starts Supabase Gmail OAuth.
3. Supabase returns a session to the SPA.
4. SPA stores session state using Supabase client behavior.
5. SPA sends backend requests with `Authorization: Bearer <supabase_access_token>`.

## Backend

### Stack

- Stable Rust.
- HTTP API server.
- PostgreSQL connection pool.
- SQL migrations managed outside Supabase-specific tooling.
- Structured logging and request tracing.

### Responsibilities

- Verify Supabase JWTs on protected API routes.
- Map authenticated Supabase users to internal application users.
- Own all application-specific persistence.
- Serve stock ticker search and one-month chart data endpoints.
- Proxy or normalize market-data API responses so the frontend is not tied to one provider.
- Enforce authorization for user-specific data.

### Suggested Structure

```text
src/
  main.rs
  config.rs
  http/
    mod.rs
    auth.rs
    health.rs
    stocks.rs
  domain/
    users.rs
    stocks.rs
  db/
    mod.rs
    migrations/
    users.rs
  integrations/
    supabase_auth.rs
    market_data.rs
```

### API Shape

- `GET /healthz`
  - Public health check.

- `GET /api/me`
  - Protected.
  - Returns the current internal user profile.
  - Creates or updates the internal user record if this is the first authenticated request.

- `GET /api/stocks/search?q=<query>`
  - Protected.
  - Searches ticker symbols through the configured market-data provider.

- `GET /api/stocks/{ticker}/prices?range=1mo`
  - Protected.
  - Returns normalized price history for charting.

## Database

### Database Rule

- PostgreSQL is the application database.
- Supabase-hosted Postgres is allowed only as a normal PostgreSQL host.
- The application must avoid Supabase-specific storage assumptions such as relying on Supabase row-level security, Supabase database functions, or Supabase-generated schemas for core app behavior.

### Initial Tables

```text
users
  id uuid primary key
  supabase_user_id text unique not null
  email text not null
  created_at timestamptz not null
  updated_at timestamptz not null

stock_watchlist_items
  id uuid primary key
  user_id uuid references users(id)
  ticker text not null
  created_at timestamptz not null
```

## Market Data

- Market-data access should go through a backend interface.
- Provider-specific code belongs in `integrations/market_data.rs`.
- Frontend chart data should use a normalized response format.

Example normalized price point:

```json
{
  "date": "2026-06-05",
  "open": 100.0,
  "high": 102.0,
  "low": 99.0,
  "close": 101.0,
  "volume": 1234567
}
```

## Configuration

Backend configuration should come from environment variables:

- `DATABASE_URL`
- `SUPABASE_PROJECT_URL`
- `SUPABASE_JWT_ISSUER`
- `SUPABASE_JWT_AUDIENCE`
- `SUPABASE_JWT_JWKS_URL`
- `MARKET_DATA_PROVIDER`
- `MARKET_DATA_API_KEY`, if the selected provider needs one

Frontend configuration should come from build/runtime environment:

- `VITE_SUPABASE_URL`
- `VITE_SUPABASE_ANON_KEY`
- `VITE_API_BASE_URL`

## Implementation Notes

- The backend should verify JWTs directly using Supabase JWKS instead of trusting user IDs sent by the frontend.
- The frontend should never call the database directly.
- The frontend should avoid direct market-data provider calls unless the provider is intentionally public and rate-limit-safe.
- Database migrations should be portable SQL.
- The stock provider should be swappable without changing Svelte components.
