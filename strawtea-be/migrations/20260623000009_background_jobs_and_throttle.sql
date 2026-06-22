create table provider_usage_windows (
  provider text not null,
  window_kind text not null,
  window_start timestamptz not null,
  request_count integer not null default 0,
  credit_count integer not null default 0,
  updated_at timestamptz not null default now(),
  primary key (provider, window_kind, window_start),
  constraint provider_usage_windows_provider_check
    check (provider in ('twelvedata', 'sec_edgar')),
  constraint provider_usage_windows_window_kind_check
    check (window_kind in ('second', 'minute', 'day')),
  constraint provider_usage_windows_counts_check
    check (request_count >= 0 and credit_count >= 0)
);

create table background_jobs (
  id uuid primary key default gen_random_uuid(),
  user_id uuid references users(id) on delete cascade,
  job_type text not null,
  status text not null,
  run_after timestamptz not null default now(),
  status_reason text,
  error text,
  progress_current integer not null default 0,
  progress_total integer not null default 0,
  payload jsonb not null default '{}'::jsonb,
  started_at timestamptz,
  completed_at timestamptz,
  stopped_at timestamptz,
  aborted_at timestamptz,
  failed_at timestamptz,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  constraint background_jobs_status_check
    check (status in ('queued', 'running', 'waiting_rate_limit', 'stopped', 'completed', 'failed', 'aborted'))
);

create index background_jobs_runnable_idx
  on background_jobs (job_type, status, run_after, created_at);

create index background_jobs_user_created_idx
  on background_jobs (user_id, created_at desc);

create table background_job_items (
  id uuid primary key default gen_random_uuid(),
  job_id uuid not null references background_jobs(id) on delete cascade,
  item_key text not null,
  status text not null,
  attempts integer not null default 0,
  payload jsonb not null default '{}'::jsonb,
  result_ref uuid,
  last_error text,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  unique (job_id, item_key),
  constraint background_job_items_status_check
    check (status in ('queued', 'processing', 'completed', 'skipped', 'failed'))
);

create index background_job_items_job_status_idx
  on background_job_items (job_id, status, created_at);

alter table ai_screener_runs
  drop constraint if exists ai_screener_runs_status_check;

alter table ai_screener_runs
  add column job_id uuid references background_jobs(id) on delete set null,
  add column run_after timestamptz,
  add column status_reason text;

alter table ai_screener_runs
  add constraint ai_screener_runs_status_check
    check (status in ('queued', 'running', 'waiting_rate_limit', 'stopped', 'completed', 'failed', 'aborted'));

create unique index ai_screener_runs_job_id_idx
  on ai_screener_runs (job_id)
  where job_id is not null;
