alter table investlog_watchlist
  add column shares_outstanding bigint,
  add column revenue bigint,
  add column total_debt bigint,
  add column cash bigint,
  add column free_cash_flow bigint,
  add constraint investlog_watchlist_shares_outstanding_positive
    check (shares_outstanding is null or shares_outstanding > 0);
