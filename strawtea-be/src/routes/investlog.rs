use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, post, put},
};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    error::AppError,
    models::{
        AddWatchlistItem, CreateInvestlogEntry, CreateInvestlogTickerNote, InvestlogAsset,
        InvestlogAssets, InvestlogAssetsSummary, InvestlogEntry, InvestlogPerformance,
        InvestlogPerformanceEvent, InvestlogPerformancePoint, InvestlogPerformanceSeries,
        InvestlogReportEvent, InvestlogTickerNote, InvestlogWatchlistItem, PricePoint,
        UpdateInvestlogEntry, WatchlistRemoval,
    },
    state::AppState,
};

pub fn investlog_routes() -> Router<AppState> {
    Router::new()
        .route("/investlog", get(list_entries).post(create_entry))
        .route("/investlog/{id}", put(update_entry))
        .route("/investlog/assets", get(list_assets))
        .route(
            "/investlog/watchlist",
            get(list_watchlist).post(add_watchlist_item),
        )
        .route(
            "/investlog/watchlist/{ticker}/remove",
            post(remove_watchlist_item),
        )
        .route("/investlog/notes", post(create_ticker_note))
        .route("/investlog/performance", get(performance))
}

#[derive(Deserialize)]
struct PerformanceQuery {
    ticker: String,
    range: Option<String>,
}

#[derive(Clone, Copy)]
struct PerformanceRange {
    label: &'static str,
    days: i64,
    output_size: u16,
}

async fn list_entries(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<InvestlogEntry>>, AppError> {
    let user_id = current_user_id(&state, &headers).await?;

    let rows = sqlx::query_as::<_, InvestlogRow>(
        r#"
        select
          id,
          ticker,
          occurred_at,
          op::text as op,
          broker::text as broker,
          currency::text as currency,
          price,
          quantity,
          fees,
          notes,
          created_at,
          updated_at
        from investlog
        where user_id = $1
        order by occurred_at desc, created_at desc
        "#,
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(rows.into_iter().map(Into::into).collect()))
}

async fn create_entry(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateInvestlogEntry>,
) -> Result<Json<InvestlogEntry>, AppError> {
    let user_id = current_user_id(&state, &headers).await?;
    validate_entry(&payload)?;

    let ticker = payload.ticker.trim().to_uppercase();
    let notes = payload.notes.trim();

    let row = sqlx::query_as::<_, InvestlogRow>(
        r#"
        insert into investlog (
          user_id,
          ticker,
          occurred_at,
          op,
          broker,
          currency,
          price,
          quantity,
          fees,
          notes
        )
        values (
          $1,
          $2,
          $3,
          $4::investlog_op,
          $5::investlog_broker,
          $6::investlog_currency,
          $7,
          $8,
          $9,
          $10
        )
        returning
          id,
          ticker,
          occurred_at,
          op::text as op,
          broker::text as broker,
          currency::text as currency,
          price,
          quantity,
          fees,
          notes,
          created_at,
          updated_at
        "#,
    )
    .bind(user_id)
    .bind(ticker)
    .bind(payload.occurred_at)
    .bind(payload.op.as_str())
    .bind(payload.broker.as_str())
    .bind(payload.currency.as_str())
    .bind(payload.price)
    .bind(payload.quantity)
    .bind(payload.fees)
    .bind(notes)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(row.into()))
}

async fn update_entry(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateInvestlogEntry>,
) -> Result<Json<InvestlogEntry>, AppError> {
    let user_id = current_user_id(&state, &headers).await?;
    validate_entry(&payload)?;

    let ticker = payload.ticker.trim().to_uppercase();
    let notes = payload.notes.trim();

    let row = sqlx::query_as::<_, InvestlogRow>(
        r#"
        update investlog
        set ticker = $3,
            occurred_at = $4,
            op = $5::investlog_op,
            broker = $6::investlog_broker,
            currency = $7::investlog_currency,
            price = $8,
            quantity = $9,
            fees = $10,
            notes = $11,
            updated_at = now()
        where user_id = $1
          and id = $2
        returning
          id,
          ticker,
          occurred_at,
          op::text as op,
          broker::text as broker,
          currency::text as currency,
          price,
          quantity,
          fees,
          notes,
          created_at,
          updated_at
        "#,
    )
    .bind(user_id)
    .bind(id)
    .bind(ticker)
    .bind(payload.occurred_at)
    .bind(payload.op.as_str())
    .bind(payload.broker.as_str())
    .bind(payload.currency.as_str())
    .bind(payload.price)
    .bind(payload.quantity)
    .bind(payload.fees)
    .bind(notes)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("investlog entry not found".to_string()))?;

    Ok(Json(row.into()))
}

async fn list_assets(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<InvestlogAssets>, AppError> {
    let user_id = current_user_id(&state, &headers).await?;
    let summary_row = sqlx::query_as::<_, AssetSummaryRow>(
        r#"
        select
          coalesce(sum(((price * quantity + 50) / 100)) filter (where op = 'buy'), 0)::bigint as total_buys,
          coalesce(sum(((price * quantity + 50) / 100)) filter (where op = 'sell'), 0)::bigint as total_sells,
          coalesce(sum(fees), 0)::bigint as total_commissions,
          coalesce(sum(fees) filter (where op = 'buy'), 0)::bigint as buy_commissions,
          coalesce(sum(fees) filter (where op = 'sell'), 0)::bigint as sell_commissions
        from investlog
        where user_id = $1
        "#,
    )
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;

    let rows = sqlx::query_as::<_, AssetRow>(
        r#"
        select
          ticker,
          sum(case when op = 'buy' then quantity else -quantity end)::bigint as quantity,
          sum(case when op = 'buy' then ((price * quantity + 50) / 100) + fees else 0 end)::bigint as buy_cost,
          sum(case when op = 'buy' then quantity else 0 end)::bigint as buy_quantity,
          (min(occurred_at) filter (where op = 'buy'))::date as first_buy_date,
          (max(occurred_at) filter (where op = 'buy'))::date as last_buy_date
        from investlog
        where user_id = $1
        group by ticker
        having sum(case when op = 'buy' then quantity else -quantity end) > 0
        order by ticker
        "#,
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;

    let mut assets = Vec::with_capacity(rows.len());
    let today = Utc::now().date_naive();
    let mut current_open_value = 0;
    let mut open_cost_basis = 0;

    for row in rows {
        let cached_price = cached_or_refreshed_price(&state, &row.ticker).await?;
        let avg_buy_price = rounded_div(row.buy_cost * 100, row.buy_quantity);
        let price_change = cached_price.price - avg_buy_price;
        let cost = rounded_div(avg_buy_price * row.quantity, 100);
        let current_value = rounded_div(cached_price.price * row.quantity, 100);
        open_cost_basis += cost;
        current_open_value += current_value;
        let amount_change = current_value - cost;
        let buy_span_days = row
            .last_buy_date
            .signed_duration_since(row.first_buy_date)
            .num_days();
        let buy_midpoint_date = row.first_buy_date + Duration::days(buy_span_days / 2);
        let days_since_buy_midpoint = today.signed_duration_since(buy_midpoint_date).num_days();
        let percent_change = if cost == 0 {
            0.0
        } else {
            (amount_change as f64 / cost as f64) * 100.0
        };

        assets.push(InvestlogAsset {
            ticker: row.ticker,
            days_since_buy_midpoint,
            quantity: row.quantity,
            avg_buy_price,
            cost,
            current_price: cached_price.price,
            price_change,
            current_value,
            amount_change,
            percent_change,
            price_fetched_at: cached_price.fetched_at,
        });
    }

    let total_buy_cost_basis = summary_row.total_buys + summary_row.buy_commissions;
    let closed_cost_basis = (total_buy_cost_basis - open_cost_basis).max(0);
    let realized_profit =
        summary_row.total_sells - summary_row.sell_commissions - closed_cost_basis;
    let unrealized_profit = current_open_value - open_cost_basis;
    let summary = InvestlogAssetsSummary {
        total_buys: summary_row.total_buys,
        total_sells: summary_row.total_sells,
        total_commissions: summary_row.total_commissions,
        realized_profit,
        unrealized_profit,
        net_profit: realized_profit + unrealized_profit,
    };

    Ok(Json(InvestlogAssets { summary, assets }))
}

async fn list_watchlist(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<InvestlogWatchlistItem>>, AppError> {
    let user_id = current_user_id(&state, &headers).await?;
    refresh_watchlist_cache(&state, user_id).await?;

    let mut rows = sqlx::query_as::<_, WatchlistRow>(
        r#"
        select
          id,
          ticker,
          company_name,
          description,
          market_cap,
          shares_outstanding,
          revenue,
          total_debt,
          cash,
          free_cash_flow,
          current_price,
          currency,
          removed_at is null as is_active,
          removed_at,
          meta_fetched_at,
          price_fetched_at,
          created_at,
          updated_at
        from investlog_watchlist
        where user_id = $1
        order by removed_at is not null, updated_at desc, ticker
        "#,
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;

    attach_ticker_notes(&state, user_id, &mut rows).await?;
    Ok(Json(rows.into_iter().map(Into::into).collect()))
}

async fn refresh_watchlist_cache(state: &AppState, user_id: Uuid) -> Result<(), AppError> {
    let rows = sqlx::query_as::<_, WatchlistCacheRow>(
        r#"
        select ticker, shares_outstanding, meta_fetched_at, price_fetched_at
        from investlog_watchlist
        where user_id = $1
          and removed_at is null
        order by updated_at desc
        "#,
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;

    let now = Utc::now();
    for row in rows {
        let meta_stale = row
            .meta_fetched_at
            .map(|fetched_at| now - fetched_at > Duration::hours(24))
            .unwrap_or(true);
        let price_stale = row
            .price_fetched_at
            .map(|fetched_at| now - fetched_at > Duration::minutes(10))
            .unwrap_or(true);

        if meta_stale {
            match state
                .edgar
                .company_profile_summary(&crate::integrations::edgar::EdgarTickerCompany {
                    ticker: row.ticker.clone(),
                    name: row.ticker.clone(),
                })
                .await
            {
                Ok(profile) => {
                    sqlx::query(
                        r#"
                        update investlog_watchlist
                        set company_name = $3,
                            description = $4,
                            meta_fetched_at = now(),
                            updated_at = now()
                        where user_id = $1
                          and ticker = $2
                        "#,
                    )
                    .bind(user_id)
                    .bind(&row.ticker)
                    .bind(profile.name)
                    .bind(profile.sic_description)
                    .execute(&state.db)
                    .await?;
                }
                Err(err @ AppError::RateLimited { .. }) => return Err(err),
                Err(err) => {
                    tracing::debug!(
                        ticker = %row.ticker,
                        error = %err,
                        "watchlist metadata refresh failed"
                    );
                }
            }
        }

        if price_stale {
            match state.market_data.latest_price_cents(&row.ticker).await {
                Ok(price) => {
                    let should_refresh_financials = meta_stale || row.shares_outstanding.is_none();
                    let financials = if should_refresh_financials {
                        match state.edgar.financial_snapshot(&row.ticker).await {
                            Ok(value) => Some(value),
                            Err(AppError::RateLimited { .. }) => None,
                            Err(err) => {
                                tracing::debug!(
                                    ticker = %row.ticker,
                                    error = %err,
                                    "watchlist financial snapshot refresh failed"
                                );
                                None
                            }
                        }
                    } else {
                        None
                    };
                    let shares_outstanding = financials
                        .as_ref()
                        .and_then(|snapshot| snapshot.shares_outstanding);
                    let market_cap = shares_outstanding.map(|shares| {
                        ((shares as i128 * price as i128).min(i64::MAX as i128)) as i64
                    });

                    sqlx::query(
                        r#"
                        update investlog_watchlist
                        set current_price = $3,
                            market_cap = coalesce($4, market_cap),
                            shares_outstanding = coalesce($5, shares_outstanding),
                            revenue = coalesce($6, revenue),
                            total_debt = coalesce($7, total_debt),
                            cash = coalesce($8, cash),
                            free_cash_flow = coalesce($9, free_cash_flow),
                            currency = 'USD',
                            price_fetched_at = now(),
                            updated_at = now()
                        where user_id = $1
                          and ticker = $2
                        "#,
                    )
                    .bind(user_id)
                    .bind(&row.ticker)
                    .bind(price)
                    .bind(market_cap)
                    .bind(shares_outstanding)
                    .bind(financials.as_ref().and_then(|snapshot| snapshot.revenue))
                    .bind(financials.as_ref().and_then(|snapshot| snapshot.total_debt))
                    .bind(financials.as_ref().and_then(|snapshot| snapshot.cash))
                    .bind(
                        financials
                            .as_ref()
                            .and_then(|snapshot| snapshot.free_cash_flow),
                    )
                    .execute(&state.db)
                    .await?;
                }
                Err(err @ AppError::RateLimited { .. }) => return Err(err),
                Err(err) => {
                    tracing::debug!(
                        ticker = %row.ticker,
                        error = %err,
                        "watchlist price refresh failed"
                    );
                }
            }
        }
    }

    Ok(())
}

async fn insert_ticker_note(
    state: &AppState,
    user_id: Uuid,
    ticker: &str,
    note: &str,
) -> Result<InvestlogTickerNote, AppError> {
    let row = sqlx::query_as::<_, TickerNoteRow>(
        r#"
        insert into investlog_ticker_notes (
          user_id,
          ticker,
          note
        )
        values ($1, $2, $3)
        returning id, ticker, note, created_at
        "#,
    )
    .bind(user_id)
    .bind(ticker)
    .bind(note)
    .fetch_one(&state.db)
    .await?;

    Ok(row.into())
}

async fn ticker_notes(
    state: &AppState,
    user_id: Uuid,
    tickers: &[String],
) -> Result<Vec<TickerNoteRow>, AppError> {
    if tickers.is_empty() {
        return Ok(Vec::new());
    }

    let notes = sqlx::query_as::<_, TickerNoteRow>(
        r#"
        select id, ticker, note, created_at
        from investlog_ticker_notes
        where user_id = $1
          and ticker = any($2)
        order by created_at desc
        "#,
    )
    .bind(user_id)
    .bind(tickers)
    .fetch_all(&state.db)
    .await?;

    Ok(notes)
}

async fn attach_ticker_notes(
    state: &AppState,
    user_id: Uuid,
    rows: &mut [WatchlistRow],
) -> Result<(), AppError> {
    let tickers = rows
        .iter()
        .map(|row| row.ticker.clone())
        .collect::<Vec<_>>();
    let notes = ticker_notes(state, user_id, &tickers).await?;

    for row in rows {
        row.notes = notes
            .iter()
            .filter(|note| note.ticker == row.ticker)
            .cloned()
            .collect();
    }

    Ok(())
}

async fn add_watchlist_item(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<AddWatchlistItem>,
) -> Result<Json<InvestlogWatchlistItem>, AppError> {
    let user_id = current_user_id(&state, &headers).await?;
    let (ticker, note) = parse_watchlist_note(&payload)?;

    let row = sqlx::query_as::<_, WatchlistRow>(
        r#"
        insert into investlog_watchlist (
          user_id,
          ticker,
          removed_at,
          updated_at
        )
        values ($1, $2, null, now())
        on conflict (user_id, ticker) do update
        set removed_at = null,
            updated_at = now()
        returning
          id,
          ticker,
          company_name,
          description,
          market_cap,
          shares_outstanding,
          revenue,
          total_debt,
          cash,
          free_cash_flow,
          current_price,
          currency,
          removed_at is null as is_active,
          removed_at,
          meta_fetched_at,
          price_fetched_at,
          created_at,
          updated_at
        "#,
    )
    .bind(user_id)
    .bind(&ticker)
    .fetch_one(&state.db)
    .await?;

    let note = format!("star: {note}");
    insert_ticker_note(&state, user_id, &ticker, &note).await?;
    let mut rows = vec![row];
    attach_ticker_notes(&state, user_id, &mut rows).await?;
    let row = rows
        .into_iter()
        .next()
        .expect("watchlist row should exist after insert");

    Ok(Json(row.into()))
}

async fn remove_watchlist_item(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(ticker): Path<String>,
    Json(payload): Json<WatchlistRemoval>,
) -> Result<Json<InvestlogWatchlistItem>, AppError> {
    let user_id = current_user_id(&state, &headers).await?;
    let ticker = normalize_ticker(&ticker)?;
    let note = validate_note(&payload.note)?;

    let row = sqlx::query_as::<_, WatchlistRow>(
        r#"
        update investlog_watchlist
        set removed_at = now(),
            updated_at = now()
        where user_id = $1
          and ticker = $2
          and removed_at is null
        returning
          id,
          ticker,
          company_name,
          description,
          market_cap,
          shares_outstanding,
          revenue,
          total_debt,
          cash,
          free_cash_flow,
          current_price,
          currency,
          removed_at is null as is_active,
          removed_at,
          meta_fetched_at,
          price_fetched_at,
          created_at,
          updated_at
        "#,
    )
    .bind(user_id)
    .bind(ticker.clone())
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("{ticker} is not active in watchlist")))?;

    let note = format!("unstar: {note}");
    insert_ticker_note(&state, user_id, &ticker, &note).await?;
    let mut rows = vec![row];
    attach_ticker_notes(&state, user_id, &mut rows).await?;
    let row = rows
        .into_iter()
        .next()
        .expect("watchlist row should exist after update");

    Ok(Json(row.into()))
}

async fn create_ticker_note(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateInvestlogTickerNote>,
) -> Result<Json<InvestlogTickerNote>, AppError> {
    let user_id = current_user_id(&state, &headers).await?;
    let ticker = normalize_ticker(&payload.ticker)?;
    let note = validate_note(&payload.note)?;
    let note = insert_ticker_note(&state, user_id, &ticker, &note).await?;

    Ok(Json(note))
}

async fn performance(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<PerformanceQuery>,
) -> Result<Json<InvestlogPerformance>, AppError> {
    let user_id = current_user_id(&state, &headers).await?;
    let tickers = parse_tickers(&query.ticker)?;
    let requested_range = query.range.unwrap_or_else(|| "1y".to_string());
    let range = performance_range(&requested_range)?;

    let start_date = Utc::now().date_naive() - Duration::days(range.days);
    let mut series = Vec::with_capacity(tickers.len());

    for ticker in &tickers {
        let prices = cached_or_refreshed_daily_prices(&state, ticker, range, start_date).await?;
        let Some(first_price) = prices.first() else {
            series.push(InvestlogPerformanceSeries {
                ticker: ticker.clone(),
                points: Vec::new(),
            });
            continue;
        };

        let first_close = first_price.close as f64;
        let points = prices
            .into_iter()
            .map(|price| InvestlogPerformancePoint {
                date: price.date,
                close: price.close,
                index: (price.close as f64 / first_close) * 100.0,
            })
            .collect();

        series.push(InvestlogPerformanceSeries {
            ticker: ticker.clone(),
            points,
        });
    }

    let events = sqlx::query_as::<_, PerformanceEventRow>(
        r#"
        select
          ticker,
          occurred_at::date as date,
          op::text as op,
          price,
          quantity,
          notes
        from investlog
        where user_id = $1
          and ticker = any($2)
          and occurred_at::date >= $3
        order by occurred_at
        "#,
    )
    .bind(user_id)
    .bind(&tickers)
    .bind(start_date)
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(Into::into)
    .collect();
    let report_events = performance_report_events(&state, &tickers, start_date).await;
    let ticker_notes = ticker_notes(&state, user_id, &tickers)
        .await?
        .into_iter()
        .map(Into::into)
        .collect();

    Ok(Json(InvestlogPerformance {
        tickers,
        range: range.label.to_string(),
        series,
        events,
        report_events,
        ticker_notes,
    }))
}

async fn performance_report_events(
    state: &AppState,
    tickers: &[String],
    start_date: NaiveDate,
) -> Vec<InvestlogReportEvent> {
    let mut events = Vec::new();

    for ticker in tickers {
        if let Ok(mut ticker_events) = state.edgar.report_events(ticker, start_date).await {
            events.extend(ticker_events.drain(..).map(|event| InvestlogReportEvent {
                ticker: event.ticker,
                date: event.date,
                form: event.form,
                filing_date: event.filing_date,
            }));
        }
    }

    events.sort_by(|left, right| {
        left.date
            .cmp(&right.date)
            .then_with(|| left.ticker.cmp(&right.ticker))
            .then_with(|| left.form.cmp(&right.form))
    });
    events
}

async fn cached_or_refreshed_price(
    state: &AppState,
    ticker: &str,
) -> Result<CachedPrice, AppError> {
    if let Some(row) = sqlx::query_as::<_, CachedPrice>(
        r#"
        select price, fetched_at
        from ticker_price_cache
        where ticker = $1
          and fetched_at > now() - interval '15 minutes'
        "#,
    )
    .bind(ticker)
    .fetch_optional(&state.db)
    .await?
    {
        return Ok(row);
    }

    let price = state.market_data.latest_price_cents(ticker).await?;

    let row = sqlx::query_as::<_, CachedPrice>(
        r#"
        insert into ticker_price_cache (
          ticker,
          price,
          currency,
          provider,
          fetched_at,
          updated_at
        )
        values ($1, $2, 'USD', 'twelvedata', now(), now())
        on conflict (ticker) do update
        set price = excluded.price,
            currency = excluded.currency,
            provider = excluded.provider,
            fetched_at = excluded.fetched_at,
            updated_at = now()
        returning price, fetched_at
        "#,
    )
    .bind(ticker)
    .bind(price)
    .fetch_one(&state.db)
    .await?;

    Ok(row)
}

async fn cached_or_refreshed_daily_prices(
    state: &AppState,
    ticker: &str,
    range: PerformanceRange,
    start_date: NaiveDate,
) -> Result<Vec<DailyPriceRow>, AppError> {
    let stats = sqlx::query_as::<_, DailyPriceCacheStats>(
        r#"
        select
          count(*)::bigint as count,
          min(date) as earliest_date,
          max(date) as latest_date,
          max(fetched_at) filter (where date >= current_date - 7) as recent_fetched_at
        from ticker_daily_price_cache
        where ticker = $1
          and provider = 'twelvedata'
          and date >= $2
        "#,
    )
    .bind(ticker)
    .bind(start_date)
    .fetch_one(&state.db)
    .await?;

    let today = Utc::now().date_naive();
    let earliest_is_late = stats
        .earliest_date
        .map(|date| date > start_date + Duration::days(5))
        .unwrap_or(true);
    let recent_is_stale = stats
        .recent_fetched_at
        .map(|fetched_at| Utc::now() - fetched_at > Duration::minutes(15))
        .unwrap_or(true);
    let latest_is_old = stats
        .latest_date
        .map(|date| date < today - Duration::days(5))
        .unwrap_or(true);

    if stats.count == 0 || earliest_is_late || latest_is_old || recent_is_stale {
        let prices = state
            .market_data
            .daily_price_history(ticker, range.output_size)
            .await?;
        upsert_daily_prices(state, ticker, prices).await?;
    }

    sqlx::query_as::<_, DailyPriceRow>(
        r#"
        select date, close
        from ticker_daily_price_cache
        where ticker = $1
          and provider = 'twelvedata'
          and date >= $2
        order by date
        "#,
    )
    .bind(ticker)
    .bind(start_date)
    .fetch_all(&state.db)
    .await
    .map_err(Into::into)
}

async fn upsert_daily_prices(
    state: &AppState,
    ticker: &str,
    prices: Vec<PricePoint>,
) -> Result<(), AppError> {
    for price in prices {
        sqlx::query(
            r#"
            insert into ticker_daily_price_cache (
              ticker,
              date,
              open,
              high,
              low,
              close,
              volume,
              currency,
              provider,
              fetched_at,
              updated_at
            )
            values ($1, $2, $3, $4, $5, $6, $7, 'USD', 'twelvedata', now(), now())
            on conflict (ticker, date, provider) do update
            set open = excluded.open,
                high = excluded.high,
                low = excluded.low,
                close = excluded.close,
                volume = excluded.volume,
                currency = excluded.currency,
                fetched_at = excluded.fetched_at,
                updated_at = now()
            "#,
        )
        .bind(ticker)
        .bind(price.date)
        .bind(f64_cents(price.open))
        .bind(f64_cents(price.high))
        .bind(f64_cents(price.low))
        .bind(f64_cents(price.close))
        .bind(price.volume.map(|volume| volume as i64))
        .execute(&state.db)
        .await?;
    }

    Ok(())
}

async fn current_user_id(state: &AppState, headers: &HeaderMap) -> Result<Uuid, AppError> {
    let auth_user = state.auth.user_from_headers(headers)?;

    let row: (Uuid,) = sqlx::query_as(
        r#"
        insert into users (supabase_user_id, email)
        values ($1, $2)
        on conflict (supabase_user_id) do update
        set email = excluded.email,
            updated_at = now()
        returning id
        "#,
    )
    .bind(auth_user.supabase_user_id)
    .bind(auth_user.email)
    .fetch_one(&state.db)
    .await?;

    Ok(row.0)
}

fn validate_entry(entry: &CreateInvestlogEntry) -> Result<(), AppError> {
    if entry.ticker.trim().is_empty() {
        return Err(AppError::BadRequest("ticker is required".to_string()));
    }

    if !matches!(entry.op.as_str(), "buy" | "sell") {
        return Err(AppError::BadRequest("op must be buy or sell".to_string()));
    }

    if entry.broker != "minvest" {
        return Err(AppError::BadRequest("broker must be minvest".to_string()));
    }

    if entry.currency != "USD" {
        return Err(AppError::BadRequest("currency must be USD".to_string()));
    }

    if entry.price <= 0 {
        return Err(AppError::BadRequest("price must be positive".to_string()));
    }

    if entry.quantity <= 0 {
        return Err(AppError::BadRequest(
            "quantity must be positive".to_string(),
        ));
    }

    if entry.fees < 0 {
        return Err(AppError::BadRequest("fees cannot be negative".to_string()));
    }

    if entry.notes.trim().is_empty() {
        return Err(AppError::BadRequest("notes are required".to_string()));
    }

    Ok(())
}

fn parse_watchlist_note(payload: &AddWatchlistItem) -> Result<(String, String), AppError> {
    Ok((
        normalize_ticker(&payload.ticker)?,
        validate_note(&payload.note)?,
    ))
}

fn normalize_ticker(value: &str) -> Result<String, AppError> {
    let ticker = value.trim().replace('.', "-").to_uppercase();
    if ticker.is_empty() {
        return Err(AppError::BadRequest("ticker is required".to_string()));
    }

    Ok(ticker)
}

fn validate_note(value: &str) -> Result<String, AppError> {
    let note = value.trim().to_string();
    if note.is_empty() {
        return Err(AppError::BadRequest("note is required".to_string()));
    }

    Ok(note)
}

#[derive(sqlx::FromRow)]
struct InvestlogRow {
    id: Uuid,
    ticker: String,
    occurred_at: DateTime<Utc>,
    op: String,
    broker: String,
    currency: String,
    price: i64,
    quantity: i64,
    fees: i64,
    notes: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct AssetRow {
    ticker: String,
    quantity: i64,
    buy_cost: i64,
    buy_quantity: i64,
    first_buy_date: NaiveDate,
    last_buy_date: NaiveDate,
}

#[derive(sqlx::FromRow)]
struct AssetSummaryRow {
    total_buys: i64,
    total_sells: i64,
    total_commissions: i64,
    buy_commissions: i64,
    sell_commissions: i64,
}

#[derive(sqlx::FromRow)]
struct WatchlistRow {
    id: Uuid,
    ticker: String,
    company_name: Option<String>,
    description: Option<String>,
    market_cap: Option<i64>,
    shares_outstanding: Option<i64>,
    revenue: Option<i64>,
    total_debt: Option<i64>,
    cash: Option<i64>,
    free_cash_flow: Option<i64>,
    current_price: Option<i64>,
    currency: Option<String>,
    is_active: bool,
    removed_at: Option<DateTime<Utc>>,
    meta_fetched_at: Option<DateTime<Utc>>,
    price_fetched_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    #[sqlx(skip)]
    notes: Vec<TickerNoteRow>,
}

#[derive(Clone, sqlx::FromRow)]
struct TickerNoteRow {
    id: Uuid,
    ticker: String,
    note: String,
    created_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct WatchlistCacheRow {
    ticker: String,
    shares_outstanding: Option<i64>,
    meta_fetched_at: Option<DateTime<Utc>>,
    price_fetched_at: Option<DateTime<Utc>>,
}

#[derive(sqlx::FromRow)]
struct CachedPrice {
    price: i64,
    fetched_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct DailyPriceRow {
    date: NaiveDate,
    close: i64,
}

#[derive(sqlx::FromRow)]
struct DailyPriceCacheStats {
    count: i64,
    earliest_date: Option<NaiveDate>,
    latest_date: Option<NaiveDate>,
    recent_fetched_at: Option<DateTime<Utc>>,
}

#[derive(sqlx::FromRow)]
struct PerformanceEventRow {
    ticker: String,
    date: NaiveDate,
    op: String,
    price: i64,
    quantity: i64,
    notes: String,
}

fn rounded_div(numerator: i64, denominator: i64) -> i64 {
    if denominator == 0 {
        return 0;
    }

    (numerator + denominator / 2) / denominator
}

fn f64_cents(value: f64) -> i64 {
    (value * 100.0).round() as i64
}

fn performance_range(value: &str) -> Result<PerformanceRange, AppError> {
    match value {
        "1m" => Ok(PerformanceRange {
            label: "1m",
            days: 31,
            output_size: 35,
        }),
        "3m" => Ok(PerformanceRange {
            label: "3m",
            days: 93,
            output_size: 75,
        }),
        "6m" => Ok(PerformanceRange {
            label: "6m",
            days: 186,
            output_size: 140,
        }),
        "1y" => Ok(PerformanceRange {
            label: "1y",
            days: 365,
            output_size: 260,
        }),
        "3y" => Ok(PerformanceRange {
            label: "3y",
            days: 1095,
            output_size: 780,
        }),
        _ => Err(AppError::BadRequest(
            "range must be one of 1m, 3m, 6m, 1y, 3y".to_string(),
        )),
    }
}

fn parse_tickers(value: &str) -> Result<Vec<String>, AppError> {
    let mut tickers = Vec::new();

    for ticker in value.split(',') {
        let ticker = ticker.trim().to_uppercase();
        if ticker.is_empty() || tickers.contains(&ticker) {
            continue;
        }

        tickers.push(ticker);
    }

    if tickers.is_empty() {
        return Err(AppError::BadRequest("ticker is required".to_string()));
    }

    if tickers.len() > 8 {
        return Err(AppError::BadRequest(
            "at most 8 tickers can be compared".to_string(),
        ));
    }

    Ok(tickers)
}

impl From<PerformanceEventRow> for InvestlogPerformanceEvent {
    fn from(row: PerformanceEventRow) -> Self {
        Self {
            ticker: row.ticker,
            date: row.date,
            op: row.op,
            price: row.price,
            quantity: row.quantity,
            notes: row.notes,
        }
    }
}

impl From<InvestlogRow> for InvestlogEntry {
    fn from(row: InvestlogRow) -> Self {
        Self {
            id: row.id,
            ticker: row.ticker,
            occurred_at: row.occurred_at,
            op: row.op,
            broker: row.broker,
            currency: row.currency,
            price: row.price,
            quantity: row.quantity,
            fees: row.fees,
            notes: row.notes,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

impl From<WatchlistRow> for InvestlogWatchlistItem {
    fn from(row: WatchlistRow) -> Self {
        Self {
            id: row.id,
            ticker: row.ticker,
            company_name: row.company_name,
            description: row.description,
            market_cap: row.market_cap,
            shares_outstanding: row.shares_outstanding,
            revenue: row.revenue,
            total_debt: row.total_debt,
            cash: row.cash,
            free_cash_flow: row.free_cash_flow,
            current_price: row.current_price,
            currency: row.currency,
            notes: row.notes.into_iter().map(Into::into).collect(),
            is_active: row.is_active,
            removed_at: row.removed_at,
            meta_fetched_at: row.meta_fetched_at,
            price_fetched_at: row.price_fetched_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

impl From<TickerNoteRow> for InvestlogTickerNote {
    fn from(row: TickerNoteRow) -> Self {
        Self {
            id: row.id,
            ticker: row.ticker,
            note: row.note,
            created_at: row.created_at,
        }
    }
}
