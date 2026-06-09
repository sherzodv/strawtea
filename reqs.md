# Strawtea Product Requirements

## Overview

Strawtea is a personal automation and finance application with a Rust backend and Svelte frontend. The initial product focuses on a mobile-friendly finance experience delivered as a single-page PWA.

## Technical Requirements

- The frontend must be a Svelte single-page application.
- The frontend must support PWA behavior, including installability and offline-capable app shell loading.
- Authentication must use Supabase login with Gmail as the identity provider.
- The backend must participate in authentication-sensitive flows instead of relying only on frontend-side auth state.
- Application data must be stored in PostgreSQL.
- Supabase may be used as the PostgreSQL host only if the application treats it as a plain PostgreSQL database.
- The database layer must avoid Supabase-specific database features that would prevent migration to self-hosted PostgreSQL later.
- The backend must be written in stable Rust.
- The frontend must be written with stable Svelte tooling.

## Product Requirements

### Home Page

- The first screen must behave like a phone menu.
- The home page must show large square app icons.
- Each icon must represent a feature area.
- The initial feature icon must be `Stocks`.
- Selecting the `Stocks` icon must navigate to the stocks page without a full page reload.

### Stocks Page

- The stocks page must allow the user to search for stock tickers.
- The user must be able to select a ticker from search results.
- After selecting a ticker, the page must show a price chart for that ticker.
- The initial chart range must be one month.
- Stock market data must come from open/public APIs where feasible.

## Initial User Flow

1. User opens the PWA.
2. User logs in using Gmail through Supabase authentication.
3. User lands on the home page with large square feature icons.
4. User selects the `Stocks` icon.
5. User searches for a ticker.
6. User selects a ticker.
7. User sees a one-month stock price graph.

## Open Decisions

- Which public market-data API to use for ticker search and one-month price history.
- Whether market data should be requested directly by the frontend or proxied through the Rust backend.
- PostgreSQL hosting target and connection management approach.
- Exact backend role in Supabase-authenticated requests.
