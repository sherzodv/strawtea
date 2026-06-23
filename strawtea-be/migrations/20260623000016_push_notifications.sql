create table push_subscriptions (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null references users(id) on delete cascade,
  endpoint text not null,
  p256dh text not null,
  auth text not null,
  is_active boolean not null default true,
  last_error text,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  unique (endpoint)
);

create index push_subscriptions_user_id_idx
  on push_subscriptions (user_id)
  where is_active;

create table investlog_asset_notification_state (
  user_id uuid not null references users(id) on delete cascade,
  ticker text not null,
  threshold_percent integer not null,
  avg_buy_price bigint not null,
  current_price bigint not null,
  percent_change double precision not null,
  notified_at timestamptz not null default now(),
  primary key (user_id, ticker, threshold_percent),
  constraint investlog_asset_notification_state_ticker_upper check (ticker = upper(ticker)),
  constraint investlog_asset_notification_state_threshold check (threshold_percent in (10, 20, 30)),
  constraint investlog_asset_notification_state_avg_buy_price_positive check (avg_buy_price > 0),
  constraint investlog_asset_notification_state_current_price_positive check (current_price > 0)
);
