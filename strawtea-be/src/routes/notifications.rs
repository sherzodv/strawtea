use std::collections::{HashMap, HashSet};

use axum::{
    Json, Router,
    extract::State,
    http::HeaderMap,
    routing::{get, post},
};
use chrono::{DateTime, Datelike, Timelike, Utc, Weekday};
use serde::{Deserialize, Serialize};
use tokio::time::{Duration as TokioDuration, sleep};
use uuid::Uuid;

use crate::{
    error::AppError,
    integrations::push::{PriceAlertPayload, PushNotifications},
    state::AppState,
};

const PRICE_ALERT_THRESHOLDS: [i32; 3] = [10, 20, 30];
const MARKET_REFRESH_SECONDS: u64 = 30 * 60;
const OFF_HOURS_REFRESH_SECONDS: u64 = 60 * 60;

pub fn notification_routes() -> Router<AppState> {
    Router::new()
        .route("/notifications/push-key", get(push_key))
        .route("/notifications/test", post(send_test_notification))
        .route(
            "/notifications/push-subscriptions",
            post(save_push_subscription),
        )
}

pub fn start_price_notification_supervisor(state: AppState) {
    tokio::spawn(async move {
        loop {
            if let Err(err) = refresh_asset_prices_and_notify(&state).await {
                tracing::warn!(error = %err, "asset price notification refresh failed");
            }

            sleep(refresh_interval(Utc::now())).await;
        }
    });
}

async fn push_key(State(state): State<AppState>) -> Json<PushKeyResponse> {
    Json(PushKeyResponse {
        enabled: state.push.is_some(),
        public_key: state
            .push
            .as_ref()
            .map(|push| push.public_key().to_string()),
    })
}

async fn save_push_subscription(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<SavePushSubscription>,
) -> Result<Json<PushSubscriptionResponse>, AppError> {
    if state.push.is_none() {
        return Err(AppError::BadRequest(
            "push notifications are not configured".to_string(),
        ));
    }

    validate_subscription(&payload)?;
    let user_id = current_user_id(&state, &headers).await?;

    let row: (Uuid,) = sqlx::query_as(
        r#"
        insert into push_subscriptions (
          user_id,
          endpoint,
          p256dh,
          auth,
          is_active,
          last_error
        )
        values ($1, $2, $3, $4, true, null)
        on conflict (endpoint) do update
        set user_id = excluded.user_id,
            p256dh = excluded.p256dh,
            auth = excluded.auth,
            is_active = true,
            last_error = null,
            updated_at = now()
        returning id
        "#,
    )
    .bind(user_id)
    .bind(payload.endpoint)
    .bind(payload.keys.p256dh)
    .bind(payload.keys.auth)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(PushSubscriptionResponse { id: row.0 }))
}

async fn send_test_notification(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<TestNotificationResponse>, AppError> {
    let Some(push) = &state.push else {
        return Err(AppError::BadRequest(
            "push notifications are not configured".to_string(),
        ));
    };

    let user_id = current_user_id(&state, &headers).await?;
    let subscriptions = active_push_subscriptions(&state, user_id).await?;
    let payload = PriceAlertPayload {
        title: "Strawtea test",
        body: "Push notifications are working on this device.".to_string(),
        url: "/investlog",
        ticker: "TEST",
        threshold_percent: 0,
        percent_change: 0.0,
        current_price: 100,
        avg_buy_price: 100,
    };

    let mut sent_count = 0;
    let mut failed_count = 0;

    for subscription in subscriptions {
        match send_push(push, &subscription, &payload).await {
            Ok(()) => {
                sent_count += 1;
                clear_subscription_error(&state, subscription.id).await?;
            }
            Err(err) => {
                failed_count += 1;
                let message = err.to_string();
                tracing::warn!(
                    user_id = %user_id,
                    subscription_id = %subscription.id,
                    error = %message,
                    "test push notification failed"
                );
                record_subscription_error(&state, subscription.id, &message).await?;
            }
        }
    }

    Ok(Json(TestNotificationResponse {
        sent_count,
        failed_count,
    }))
}

async fn refresh_asset_prices_and_notify(state: &AppState) -> Result<(), AppError> {
    let holdings = open_holdings(state).await?;
    if holdings.is_empty() {
        return Ok(());
    }

    let mut prices = HashMap::new();
    let tickers = holdings
        .iter()
        .map(|holding| holding.ticker.clone())
        .collect::<HashSet<_>>();

    for ticker in tickers {
        match refresh_price(state, &ticker).await {
            Ok(price) => {
                prices.insert(ticker, price);
            }
            Err(err) => {
                tracing::warn!(ticker, error = %err, "asset price refresh failed");
            }
        }
    }

    for holding in holdings {
        let Some(price) = prices.get(&holding.ticker).copied() else {
            continue;
        };

        notify_thresholds(state, holding, price).await?;
    }

    Ok(())
}

async fn open_holdings(state: &AppState) -> Result<Vec<OpenHolding>, AppError> {
    let rows = sqlx::query_as::<_, OpenHolding>(
        r#"
        select
          user_id,
          ticker,
          sum(case when op = 'buy' then quantity else -quantity end)::bigint as quantity,
          sum(case when op = 'buy' then ((price * quantity + 50) / 100) + fees else 0 end)::bigint as buy_cost,
          sum(case when op = 'buy' then quantity else 0 end)::bigint as buy_quantity
        from investlog
        group by user_id, ticker
        having sum(case when op = 'buy' then quantity else -quantity end) > 0
           and sum(case when op = 'buy' then quantity else 0 end) > 0
        order by ticker
        "#,
    )
    .fetch_all(&state.db)
    .await?;

    Ok(rows)
}

async fn refresh_price(state: &AppState, ticker: &str) -> Result<i64, AppError> {
    let price = state.market_data.latest_price_cents(ticker).await?;

    sqlx::query(
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
        "#,
    )
    .bind(ticker)
    .bind(price)
    .execute(&state.db)
    .await?;

    Ok(price)
}

async fn notify_thresholds(
    state: &AppState,
    holding: OpenHolding,
    current_price: i64,
) -> Result<(), AppError> {
    let avg_buy_price = rounded_div(holding.buy_cost * 100, holding.buy_quantity);
    if avg_buy_price <= 0 {
        return Ok(());
    }

    let percent_change = ((current_price - avg_buy_price) as f64 / avg_buy_price as f64) * 100.0;
    if percent_change < PRICE_ALERT_THRESHOLDS[0] as f64 {
        return Ok(());
    }

    for threshold in PRICE_ALERT_THRESHOLDS {
        if percent_change < threshold as f64 {
            continue;
        }

        let already_notified: bool = sqlx::query_scalar(
            r#"
            select exists (
              select 1
              from investlog_asset_notification_state
              where user_id = $1
                and ticker = $2
                and threshold_percent = $3
            )
            "#,
        )
        .bind(holding.user_id)
        .bind(&holding.ticker)
        .bind(threshold)
        .fetch_one(&state.db)
        .await?;

        if already_notified {
            continue;
        }

        let sent = send_threshold_alert(
            state,
            &holding,
            threshold,
            percent_change,
            current_price,
            avg_buy_price,
        )
        .await?;

        if sent {
            record_threshold_notification(
                state,
                &holding,
                threshold,
                avg_buy_price,
                current_price,
                percent_change,
            )
            .await?;
        }
    }

    Ok(())
}

async fn send_threshold_alert(
    state: &AppState,
    holding: &OpenHolding,
    threshold: i32,
    percent_change: f64,
    current_price: i64,
    avg_buy_price: i64,
) -> Result<bool, AppError> {
    let Some(push) = &state.push else {
        return Ok(false);
    };

    let subscriptions = active_push_subscriptions(state, holding.user_id).await?;

    if subscriptions.is_empty() {
        return Ok(false);
    }

    let body = format!(
        "{} is up {:.1}% to {}.",
        holding.ticker,
        percent_change,
        format_cents(current_price)
    );
    let payload = PriceAlertPayload {
        title: "Price alert",
        body,
        url: "/investlog",
        ticker: &holding.ticker,
        threshold_percent: threshold,
        percent_change,
        current_price,
        avg_buy_price,
    };

    let mut sent_count = 0;
    for subscription in subscriptions {
        match send_push(push, &subscription, &payload).await {
            Ok(()) => {
                sent_count += 1;
                clear_subscription_error(state, subscription.id).await?;
            }
            Err(err) => {
                let message = err.to_string();
                tracing::warn!(
                    user_id = %holding.user_id,
                    ticker = holding.ticker,
                    threshold,
                    subscription_id = %subscription.id,
                    error = %message,
                    "price alert push failed"
                );
                record_subscription_error(state, subscription.id, &message).await?;
            }
        }
    }

    Ok(sent_count > 0)
}

async fn active_push_subscriptions(
    state: &AppState,
    user_id: Uuid,
) -> Result<Vec<PushSubscriptionRow>, AppError> {
    let subscriptions = sqlx::query_as::<_, PushSubscriptionRow>(
        r#"
        select id, endpoint, p256dh, auth
        from push_subscriptions
        where user_id = $1
          and is_active
        "#,
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;

    Ok(subscriptions)
}

async fn send_push(
    push: &PushNotifications,
    subscription: &PushSubscriptionRow,
    payload: &PriceAlertPayload<'_>,
) -> anyhow::Result<()> {
    push.send_price_alert(
        &subscription.endpoint,
        &subscription.p256dh,
        &subscription.auth,
        payload,
    )
    .await
}

async fn record_threshold_notification(
    state: &AppState,
    holding: &OpenHolding,
    threshold: i32,
    avg_buy_price: i64,
    current_price: i64,
    percent_change: f64,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        insert into investlog_asset_notification_state (
          user_id,
          ticker,
          threshold_percent,
          avg_buy_price,
          current_price,
          percent_change
        )
        values ($1, $2, $3, $4, $5, $6)
        on conflict (user_id, ticker, threshold_percent) do nothing
        "#,
    )
    .bind(holding.user_id)
    .bind(&holding.ticker)
    .bind(threshold)
    .bind(avg_buy_price)
    .bind(current_price)
    .bind(percent_change)
    .execute(&state.db)
    .await?;

    Ok(())
}

async fn clear_subscription_error(state: &AppState, id: Uuid) -> Result<(), AppError> {
    sqlx::query(
        r#"
        update push_subscriptions
        set last_error = null,
            updated_at = now()
        where id = $1
        "#,
    )
    .bind(id)
    .execute(&state.db)
    .await?;

    Ok(())
}

async fn record_subscription_error(
    state: &AppState,
    id: Uuid,
    message: &str,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        update push_subscriptions
        set last_error = $2,
            updated_at = now()
        where id = $1
        "#,
    )
    .bind(id)
    .bind(message)
    .execute(&state.db)
    .await?;

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

fn validate_subscription(payload: &SavePushSubscription) -> Result<(), AppError> {
    if !payload.endpoint.starts_with("https://") {
        return Err(AppError::BadRequest(
            "push subscription endpoint must be HTTPS".to_string(),
        ));
    }

    if payload.keys.p256dh.trim().is_empty() || payload.keys.auth.trim().is_empty() {
        return Err(AppError::BadRequest(
            "push subscription keys are required".to_string(),
        ));
    }

    Ok(())
}

fn refresh_interval(now: DateTime<Utc>) -> TokioDuration {
    if is_nasdaq_working_hours(now) {
        TokioDuration::from_secs(MARKET_REFRESH_SECONDS)
    } else {
        TokioDuration::from_secs(OFF_HOURS_REFRESH_SECONDS)
    }
}

fn is_nasdaq_working_hours(now: DateTime<Utc>) -> bool {
    let eastern = now.with_timezone(&chrono_tz::America::New_York);
    let is_weekday = matches!(
        eastern.weekday(),
        Weekday::Mon | Weekday::Tue | Weekday::Wed | Weekday::Thu | Weekday::Fri
    );
    let minutes = eastern.hour() * 60 + eastern.minute();

    is_weekday && (9 * 60 + 30..16 * 60).contains(&minutes)
}

fn rounded_div(numerator: i64, denominator: i64) -> i64 {
    if denominator == 0 {
        return 0;
    }

    (numerator + denominator / 2) / denominator
}

fn format_cents(value: i64) -> String {
    format!("${:.2}", value as f64 / 100.0)
}

#[derive(Debug, Serialize)]
struct PushKeyResponse {
    enabled: bool,
    public_key: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SavePushSubscription {
    endpoint: String,
    keys: SavePushSubscriptionKeys,
}

#[derive(Debug, Deserialize)]
struct SavePushSubscriptionKeys {
    p256dh: String,
    auth: String,
}

#[derive(Debug, Serialize)]
struct PushSubscriptionResponse {
    id: Uuid,
}

#[derive(Debug, Serialize)]
struct TestNotificationResponse {
    sent_count: i32,
    failed_count: i32,
}

#[derive(sqlx::FromRow)]
struct OpenHolding {
    user_id: Uuid,
    ticker: String,
    buy_cost: i64,
    buy_quantity: i64,
}

#[derive(sqlx::FromRow)]
struct PushSubscriptionRow {
    id: Uuid,
    endpoint: String,
    p256dh: String,
    auth: String,
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::is_nasdaq_working_hours;

    #[test]
    fn detects_regular_market_hours_in_new_york_time() {
        assert!(is_nasdaq_working_hours(
            Utc.with_ymd_and_hms(2026, 6, 23, 15, 0, 0).unwrap()
        ));
        assert!(!is_nasdaq_working_hours(
            Utc.with_ymd_and_hms(2026, 6, 23, 21, 0, 0).unwrap()
        ));
        assert!(!is_nasdaq_working_hours(
            Utc.with_ymd_and_hms(2026, 6, 21, 15, 0, 0).unwrap()
        ));
    }
}
