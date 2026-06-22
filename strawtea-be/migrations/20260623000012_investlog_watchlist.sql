create table investlog_watchlist (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null references users(id) on delete cascade,
  ticker text not null,
  add_note text not null,
  remove_note text,
  removed_at timestamptz,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  constraint investlog_watchlist_ticker_upper check (ticker = upper(ticker)),
  constraint investlog_watchlist_add_note_nonempty check (length(trim(add_note)) > 0),
  constraint investlog_watchlist_remove_note_required
    check (removed_at is null or length(trim(coalesce(remove_note, ''))) > 0),
  unique (user_id, ticker)
);

create index investlog_watchlist_user_active_idx
  on investlog_watchlist (user_id, removed_at, updated_at desc);
