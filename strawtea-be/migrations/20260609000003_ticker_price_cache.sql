create table ticker_price_cache (
  ticker text primary key,
  price bigint not null,
  currency investlog_currency not null,
  provider text not null,
  provider_as_of timestamptz,
  fetched_at timestamptz not null default now(),
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  constraint ticker_price_cache_ticker_upper check (ticker = upper(ticker)),
  constraint ticker_price_cache_price_positive check (price > 0)
);
