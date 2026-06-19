use axum::{
    Json, Router,
    extract::{Query, State},
    http::HeaderMap,
    routing::get,
};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    error::AppError,
    models::{
        CreateInvestlogEntry, InvestlogAsset, InvestlogEntry, InvestlogPerformance,
        InvestlogPerformanceEvent, InvestlogPerformancePoint, InvestlogPerformanceSeries,
        PricePoint,
    },
    state::AppState,
};

pub fn investlog_routes() -> Router<AppState> {
    Router::new()
        .route("/investlog", get(list_entries).post(create_entry))
        .route("/investlog/assets", get(list_assets))
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

async fn list_assets(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<InvestlogAsset>>, AppError> {
    let user_id = current_user_id(&state, &headers).await?;

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

    for row in rows {
        let cached_price = cached_or_refreshed_price(&state, &row.ticker).await?;
        let avg_buy_price = rounded_div(row.buy_cost * 100, row.buy_quantity);
        let price_change = cached_price.price - avg_buy_price;
        let cost = rounded_div(avg_buy_price * row.quantity, 100);
        let current_value = rounded_div(cached_price.price * row.quantity, 100);
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

    Ok(Json(assets))
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

    Ok(Json(InvestlogPerformance {
        tickers,
        range: range.label.to_string(),
        series,
        events,
    }))
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
