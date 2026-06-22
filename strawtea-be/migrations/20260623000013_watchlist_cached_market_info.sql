alter table investlog_watchlist
  add column company_name text,
  add column description text,
  add column market_cap bigint,
  add column current_price bigint,
  add column currency text,
  add column meta_fetched_at timestamptz,
  add column price_fetched_at timestamptz,
  add constraint investlog_watchlist_market_cap_positive
    check (market_cap is null or market_cap > 0),
  add constraint investlog_watchlist_current_price_positive
    check (current_price is null or current_price > 0);
