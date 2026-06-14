use axum::{Json, Router, extract::State, http::HeaderMap, routing::get};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::{
    error::AppError,
    models::{CreateInvestlogEntry, InvestlogAsset, InvestlogEntry},
    state::AppState,
};

pub fn investlog_routes() -> Router<AppState> {
    Router::new()
        .route("/investlog", get(list_entries).post(create_entry))
        .route("/investlog/assets", get(list_assets))
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
          sum(case when op = 'buy' then quantity else 0 end)::bigint as buy_quantity
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

    for row in rows {
        let cached_price = cached_or_refreshed_price(&state, &row.ticker).await?;
        let avg_buy_price = rounded_div(row.buy_cost * 100, row.buy_quantity);
        let price_change = cached_price.price - avg_buy_price;
        let cost = rounded_div(avg_buy_price * row.quantity, 100);
        let current_value = rounded_div(cached_price.price * row.quantity, 100);
        let amount_change = current_value - cost;
        let percent_change = if cost == 0 {
            0.0
        } else {
            (amount_change as f64 / cost as f64) * 100.0
        };

        assets.push(InvestlogAsset {
            ticker: row.ticker,
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
}

#[derive(sqlx::FromRow)]
struct CachedPrice {
    price: i64,
    fetched_at: DateTime<Utc>,
}

fn rounded_div(numerator: i64, denominator: i64) -> i64 {
    if denominator == 0 {
        return 0;
    }

    (numerator + denominator / 2) / denominator
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
