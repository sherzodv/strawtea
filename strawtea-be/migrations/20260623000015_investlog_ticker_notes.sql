create table investlog_ticker_notes (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null references users(id) on delete cascade,
  ticker text not null,
  note text not null,
  created_at timestamptz not null default now(),
  constraint investlog_ticker_notes_ticker_upper check (ticker = upper(ticker)),
  constraint investlog_ticker_notes_note_nonempty check (length(trim(note)) > 0)
);

insert into investlog_ticker_notes (
  user_id,
  ticker,
  note,
  created_at
)
select
  user_id,
  ticker,
  'star: ' || add_note,
  created_at
from investlog_watchlist
where length(trim(add_note)) > 0;

insert into investlog_ticker_notes (
  user_id,
  ticker,
  note,
  created_at
)
select
  user_id,
  ticker,
  'unstar: ' || remove_note,
  removed_at
from investlog_watchlist
where removed_at is not null
  and length(trim(coalesce(remove_note, ''))) > 0;

create index investlog_ticker_notes_user_ticker_created_idx
  on investlog_ticker_notes (user_id, ticker, created_at desc);

alter table investlog_watchlist
  drop constraint if exists investlog_watchlist_add_note_nonempty,
  drop constraint if exists investlog_watchlist_remove_note_required;

alter table investlog_watchlist
  drop column add_note,
  drop column remove_note;
