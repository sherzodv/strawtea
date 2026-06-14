create table ticker_daily_price_cache (
  ticker text not null,
  date date not null,
  open bigint not null,
  high bigint not null,
  low bigint not null,
  close bigint not null,
  volume bigint,
  currency investlog_currency not null,
  provider text not null,
  fetched_at timestamptz not null default now(),
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  primary key (ticker, date, provider),
  constraint ticker_daily_price_cache_ticker_upper check (ticker = upper(ticker)),
  constraint ticker_daily_price_cache_open_positive check (open > 0),
  constraint ticker_daily_price_cache_high_positive check (high > 0),
  constraint ticker_daily_price_cache_low_positive check (low > 0),
  constraint ticker_daily_price_cache_close_positive check (close > 0)
);

create index ticker_daily_price_cache_ticker_date_idx
  on ticker_daily_price_cache (ticker, date);
