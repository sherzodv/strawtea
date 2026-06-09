create extension if not exists pgcrypto;

create table if not exists users (
  id uuid primary key default gen_random_uuid(),
  supabase_user_id text not null unique,
  email text not null,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);

create table if not exists stock_watchlist_items (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null references users(id) on delete cascade,
  ticker text not null,
  created_at timestamptz not null default now(),
  unique (user_id, ticker)
);

create index if not exists stock_watchlist_items_user_id_idx on stock_watchlist_items (user_id);
