create type investlog_op as enum ('buy', 'sell');
create type investlog_broker as enum ('minvest');
create type investlog_currency as enum ('USD');

create table investlog (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null references users(id) on delete cascade,
  ticker text not null,
  occurred_at timestamptz not null,
  op investlog_op not null,
  broker investlog_broker not null,
  currency investlog_currency not null,
  price bigint not null,
  quantity bigint not null,
  fees bigint not null default 0,
  notes text not null,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  constraint investlog_ticker_upper check (ticker = upper(ticker)),
  constraint investlog_price_positive check (price > 0),
  constraint investlog_quantity_positive check (quantity > 0),
  constraint investlog_fees_nonnegative check (fees >= 0),
  constraint investlog_notes_nonempty check (length(trim(notes)) > 0)
);

create index investlog_user_occurred_at_idx on investlog (user_id, occurred_at desc);
create index investlog_user_ticker_idx on investlog (user_id, ticker);
