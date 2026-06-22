alter table ai_screener_overrides
  add column manual_status text;

alter table ai_screener_overrides
  add constraint ai_screener_overrides_status_check
    check (manual_status is null or manual_status in ('Ignore', 'Watch', 'Entry Candidate', 'Rejected'));
