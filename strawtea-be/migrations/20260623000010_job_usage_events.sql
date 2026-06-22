create table background_job_provider_usage (
  job_id uuid not null references background_jobs(id) on delete cascade,
  provider text not null,
  request_count integer not null default 0,
  credit_count integer not null default 0,
  updated_at timestamptz not null default now(),
  primary key (job_id, provider),
  constraint background_job_provider_usage_provider_check
    check (provider in ('twelvedata', 'sec_edgar')),
  constraint background_job_provider_usage_counts_check
    check (request_count >= 0 and credit_count >= 0)
);

create table background_job_events (
  id uuid primary key default gen_random_uuid(),
  job_id uuid not null references background_jobs(id) on delete cascade,
  event_type text not null,
  message text not null,
  payload jsonb not null default '{}'::jsonb,
  created_at timestamptz not null default now()
);

create index background_job_events_job_created_idx
  on background_job_events (job_id, created_at desc);
