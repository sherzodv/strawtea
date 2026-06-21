create table user_settings (
  user_id uuid not null references users(id) on delete cascade,
  section text not null,
  setting_key text not null,
  value jsonb not null,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  primary key (user_id, section, setting_key),
  constraint user_settings_section_nonempty check (length(trim(section)) > 0),
  constraint user_settings_key_nonempty check (length(trim(setting_key)) > 0)
);

create index user_settings_user_section_idx on user_settings (user_id, section);

