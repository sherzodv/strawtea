create table ai_screener_runs (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null references users(id) on delete cascade,
  status text not null,
  universe_count integer not null default 0,
  processed_count integer not null default 0,
  result_count integer not null default 0,
  error text,
  started_at timestamptz,
  completed_at timestamptz,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  constraint ai_screener_runs_status_check
    check (status in ('queued', 'running', 'completed', 'failed'))
);

create index ai_screener_runs_user_created_idx
  on ai_screener_runs (user_id, created_at desc);

create table ai_screener_results (
  id uuid primary key default gen_random_uuid(),
  run_id uuid not null references ai_screener_runs(id) on delete cascade,
  ticker text not null,
  company_name text not null,
  ai_tier text,
  ai_score integer not null,
  status text not null,
  current_price double precision,
  correction_depth double precision,
  trend_distance double precision,
  momentum_condition text not null,
  volume_condition text not null,
  rejection_reason text,
  rationale jsonb not null default '{}'::jsonb,
  rank integer not null default 0,
  created_at timestamptz not null default now(),
  constraint ai_screener_results_ticker_upper check (ticker = upper(ticker)),
  constraint ai_screener_results_score_check check (ai_score between 0 and 100),
  constraint ai_screener_results_tier_check check (ai_tier is null or ai_tier in ('1', '2', '3')),
  constraint ai_screener_results_status_check
    check (status in ('Ignore', 'Watch', 'Entry Candidate', 'Rejected')),
  unique (run_id, ticker)
);

create index ai_screener_results_run_rank_idx
  on ai_screener_results (run_id, rank);

create table ai_screener_overrides (
  user_id uuid not null references users(id) on delete cascade,
  ticker text not null,
  manual_ai_tier text,
  manual_ai_score integer,
  notes text not null default '',
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  primary key (user_id, ticker),
  constraint ai_screener_overrides_ticker_upper check (ticker = upper(ticker)),
  constraint ai_screener_overrides_score_check
    check (manual_ai_score is null or manual_ai_score between 0 and 100),
  constraint ai_screener_overrides_tier_check
    check (manual_ai_tier is null or manual_ai_tier in ('1', '2', '3'))
);
