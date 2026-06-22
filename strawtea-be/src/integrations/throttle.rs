use chrono::{DateTime, Duration, Timelike, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

#[derive(Clone)]
pub struct ProviderThrottle {
    db: PgPool,
}

#[derive(Clone, Copy)]
pub struct JobBudget {
    pub job_id: Uuid,
    pub credit_limit: i32,
}

#[derive(Clone, Copy)]
struct ProviderLimit {
    window_kind: &'static str,
    max_credits: i32,
    duration: Duration,
}

impl ProviderThrottle {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    pub async fn reserve(
        &self,
        provider: &'static str,
        endpoint: &'static str,
        credit_cost: i32,
    ) -> Result<(), AppError> {
        self.reserve_with_job_budget(provider, endpoint, credit_cost, None)
            .await
    }

    pub async fn reserve_with_job_budget(
        &self,
        provider: &'static str,
        endpoint: &'static str,
        credit_cost: i32,
        job_budget: Option<JobBudget>,
    ) -> Result<(), AppError> {
        let limits = provider_limits(provider);
        let now = Utc::now();
        let mut tx = self.db.begin().await?;

        sqlx::query("select pg_advisory_xact_lock(hashtext($1)::bigint)")
            .bind(provider)
            .execute(&mut *tx)
            .await?;

        let mut retry_after: Option<DateTime<Utc>> = None;
        for limit in &limits {
            let window_start = window_start(now, limit.window_kind);
            let current = sqlx::query_as::<_, UsageWindowRow>(
                r#"
                select request_count, credit_count
                from provider_usage_windows
                where provider = $1
                  and window_kind = $2
                  and window_start = $3
                "#,
            )
            .bind(provider)
            .bind(limit.window_kind)
            .bind(window_start)
            .fetch_optional(&mut *tx)
            .await?;

            let next_credits = current
                .map(|row| row.credit_count)
                .unwrap_or(0)
                .saturating_add(credit_cost);
            if next_credits > limit.max_credits {
                let candidate = window_start + limit.duration;
                retry_after = Some(retry_after.map_or(candidate, |value| value.max(candidate)));
            }
        }

        if let Some(retry_after) = retry_after {
            return Err(AppError::RateLimited {
                provider: provider.to_string(),
                retry_after,
            });
        }

        if let Some(job_budget) = job_budget.filter(|_| provider == "twelvedata") {
            let current = sqlx::query_as::<_, JobUsageRow>(
                r#"
                select credit_count
                from background_job_provider_usage
                where job_id = $1
                  and provider = $2
                "#,
            )
            .bind(job_budget.job_id)
            .bind(provider)
            .fetch_optional(&mut *tx)
            .await?;
            let used = current.map(|row| row.credit_count).unwrap_or(0);
            let next_used = used.saturating_add(credit_cost);
            if next_used > job_budget.credit_limit {
                return Err(AppError::JobBudgetExhausted {
                    provider: provider.to_string(),
                    used,
                    limit: job_budget.credit_limit,
                });
            }
        }

        for limit in &limits {
            let window_start = window_start(now, limit.window_kind);
            sqlx::query(
                r#"
                insert into provider_usage_windows (
                  provider,
                  window_kind,
                  window_start,
                  request_count,
                  credit_count,
                  updated_at
                )
                values ($1, $2, $3, 1, $4, now())
                on conflict (provider, window_kind, window_start) do update
                set request_count = provider_usage_windows.request_count + 1,
                    credit_count = provider_usage_windows.credit_count + excluded.credit_count,
                    updated_at = now()
                "#,
            )
            .bind(provider)
            .bind(limit.window_kind)
            .bind(window_start)
            .bind(credit_cost)
            .execute(&mut *tx)
            .await?;
        }

        if let Some(job_budget) = job_budget.filter(|_| provider == "twelvedata") {
            sqlx::query(
                r#"
                insert into background_job_provider_usage (
                  job_id,
                  provider,
                  request_count,
                  credit_count,
                  updated_at
                )
                values ($1, $2, 1, $3, now())
                on conflict (job_id, provider) do update
                set request_count = background_job_provider_usage.request_count + 1,
                    credit_count = background_job_provider_usage.credit_count + excluded.credit_count,
                    updated_at = now()
                "#,
            )
            .bind(job_budget.job_id)
            .bind(provider)
            .bind(credit_cost)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        tracing::debug!(provider, endpoint, credit_cost, "provider quota reserved");
        Ok(())
    }
}

fn provider_limits(provider: &str) -> Vec<ProviderLimit> {
    match provider {
        "twelvedata" => vec![
            ProviderLimit {
                window_kind: "minute",
                max_credits: 8,
                duration: Duration::minutes(1),
            },
            ProviderLimit {
                window_kind: "day",
                max_credits: 800,
                duration: Duration::days(1),
            },
        ],
        "sec_edgar" => vec![ProviderLimit {
            window_kind: "second",
            max_credits: 8,
            duration: Duration::seconds(1),
        }],
        _ => Vec::new(),
    }
}

fn window_start(now: DateTime<Utc>, window_kind: &str) -> DateTime<Utc> {
    match window_kind {
        "second" => now.with_nanosecond(0).unwrap_or(now),
        "minute" => now
            .with_second(0)
            .and_then(|value| value.with_nanosecond(0))
            .unwrap_or(now),
        "day" => now
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .map(|value| value.and_utc())
            .unwrap_or(now),
        _ => now,
    }
}

#[derive(sqlx::FromRow)]
struct UsageWindowRow {
    credit_count: i32,
}

#[derive(sqlx::FromRow)]
struct JobUsageRow {
    credit_count: i32,
}
