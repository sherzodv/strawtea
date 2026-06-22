use std::collections::HashSet;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post, put},
};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde_json::{Value, json};
use sqlx::types::Json as SqlxJson;
use tokio::time::{Duration as TokioDuration, interval, sleep};
use uuid::Uuid;

use crate::{
    error::AppError,
    integrations::{
        edgar::EdgarTickerCompany,
        openai::{AiCompanyContext, AiEnrichment},
        throttle::JobBudget,
    },
    models::{
        AiScreenerResult, AiScreenerRun, BackgroundJobEvent, PricePoint, UpdateAiScreenerOverride,
    },
    state::AppState,
};

const MAX_DISCOVERY_NAMES: usize = 120;
const MAX_DISCOVERY_PROFILE_LOOKUPS: usize = 120;
const SCREENER_DISCOVERY_BATCH_SIZE: usize = 20;
const DISCOVERY_SEC_PROFILE_PAUSE_MS: u64 = 140;
const PRICE_OUTPUT_SIZE: u16 = 260;
const SCREENER_JOB_TYPE: &str = "ai_correction_screener";
const TWELVE_DAILY_CREDIT_LIMIT: i32 = 800;
const SCREENER_TWELVE_BUDGET_PERCENT: i32 = 10;

pub fn start_screener_supervisor(state: AppState) {
    tokio::spawn(async move {
        if let Err(err) = recover_interrupted_screener_jobs(&state).await {
            tracing::warn!(error = %err, "failed to recover interrupted screener jobs");
        }

        let mut tick = interval(TokioDuration::from_secs(15));
        loop {
            tick.tick().await;
            if let Err(err) = process_due_screener_jobs(&state).await {
                tracing::warn!(error = %err, "screener supervisor tick failed");
            }
        }
    });
}

pub fn screener_routes() -> Router<AppState> {
    Router::new()
        .route("/ai-correction-screener/runs", post(create_run))
        .route("/ai-correction-screener/runs/latest", get(latest_run))
        .route("/ai-correction-screener/runs/{id}", get(read_run))
        .route("/ai-correction-screener/jobs", post(create_run))
        .route("/ai-correction-screener/jobs/latest", get(latest_run))
        .route(
            "/ai-correction-screener/overrides/{ticker}",
            put(update_override),
        )
}

async fn create_run(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AiScreenerRun>, AppError> {
    let user_id = current_user_id(&state, &headers).await?;
    recover_user_screener_jobs(&state, user_id).await?;

    if let Some(active) = active_run(&state, user_id).await? {
        resume_job_if_paused(&state, active).await?;
        return read_run_by_id(&state, user_id, active).await.map(Json);
    }

    let job_row = sqlx::query_as::<_, JobRefRow>(
        r#"
        insert into background_jobs (
          user_id,
          job_type,
          status,
          run_after,
          payload
        )
        values ($1, $2, 'queued', now(), '{}'::jsonb)
        returning id
        "#,
    )
    .bind(user_id)
    .bind(SCREENER_JOB_TYPE)
    .fetch_one(&state.db)
    .await?;

    let run_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        insert into ai_screener_runs (user_id, job_id, status, run_after)
        values ($1, $2, 'queued', now())
        returning
          id
        "#,
    )
    .bind(user_id)
    .bind(job_row.id)
    .fetch_one(&state.db)
    .await?;

    sqlx::query(
        r#"
        update background_jobs
        set payload = jsonb_build_object('run_id', $2::text),
            updated_at = now()
        where id = $1
        "#,
    )
    .bind(job_row.id)
    .bind(run_id)
    .execute(&state.db)
    .await?;

    read_run_by_id(&state, user_id, run_id).await.map(Json)
}

async fn latest_run(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Option<AiScreenerRun>>, AppError> {
    let user_id = current_user_id(&state, &headers).await?;
    recover_user_screener_jobs(&state, user_id).await?;

    let Some(row) = sqlx::query_as::<_, RunRow>(
        r#"
        select
          id,
          job_id,
          status,
          run_after,
          status_reason,
          universe_count,
          processed_count,
          result_count,
          error,
          started_at,
          completed_at,
          created_at,
          updated_at
        from ai_screener_runs
        where user_id = $1
        order by created_at desc
        limit 1
        "#,
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?
    else {
        return Ok(Json(None));
    };

    read_run_by_id(&state, user_id, row.id)
        .await
        .map(Some)
        .map(Json)
}

async fn read_run(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<AiScreenerRun>, AppError> {
    let user_id = current_user_id(&state, &headers).await?;
    recover_user_screener_jobs(&state, user_id).await?;
    read_run_by_id(&state, user_id, id).await.map(Json)
}

async fn update_override(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(ticker): Path<String>,
    Json(payload): Json<UpdateAiScreenerOverride>,
) -> Result<Json<AiScreenerRun>, AppError> {
    let user_id = current_user_id(&state, &headers).await?;
    let ticker = normalize_ticker(&ticker)?;
    validate_override(&payload)?;

    sqlx::query(
        r#"
        insert into ai_screener_overrides (
          user_id,
          ticker,
          manual_ai_tier,
          manual_ai_score,
          manual_status,
          notes,
          updated_at
        )
        values ($1, $2, $3, $4, $5, $6, now())
        on conflict (user_id, ticker) do update
        set manual_ai_tier = excluded.manual_ai_tier,
            manual_ai_score = excluded.manual_ai_score,
            manual_status = excluded.manual_status,
            notes = excluded.notes,
            updated_at = now()
        "#,
    )
    .bind(user_id)
    .bind(ticker)
    .bind(payload.manual_ai_tier)
    .bind(payload.manual_ai_score)
    .bind(payload.manual_status)
    .bind(payload.notes.trim())
    .execute(&state.db)
    .await?;

    let Some(latest_id) = latest_run_id(&state, user_id).await? else {
        return Err(AppError::NotFound("no screener run exists".to_string()));
    };

    read_run_by_id(&state, user_id, latest_id).await.map(Json)
}

async fn process_due_screener_jobs(state: &AppState) -> Result<(), AppError> {
    let jobs = sqlx::query_as::<_, RunnableJobRow>(
        r#"
        select id, user_id, payload
        from background_jobs
        where job_type = $1
          and status in ('queued', 'waiting_rate_limit')
          and run_after <= now()
        order by created_at
        limit 3
        "#,
    )
    .bind(SCREENER_JOB_TYPE)
    .fetch_all(&state.db)
    .await?;

    for job in jobs {
        if let Err(err) = process_screener_job(state, job).await {
            tracing::warn!(error = %err, "screener job processing failed");
        }
    }

    Ok(())
}

async fn process_screener_job(state: &AppState, job: RunnableJobRow) -> Result<(), AppError> {
    let acquired = sqlx::query_scalar::<_, bool>(
        r#"
        update background_jobs
        set status = 'running',
            started_at = coalesce(started_at, now()),
            status_reason = null,
            error = null,
            updated_at = now()
        where id = $1
          and status in ('queued', 'waiting_rate_limit')
          and run_after <= now()
        returning true
        "#,
    )
    .bind(job.id)
    .fetch_optional(&state.db)
    .await?
    .unwrap_or(false);

    if !acquired {
        return Ok(());
    }
    mirror_job_to_screener_run(state, job.id).await?;

    let run_id = job_run_id(&job.payload.0)?;
    log_job_event(
        state,
        job.id,
        "job_started",
        "Screener job processing started",
        json!({}),
    )
    .await?;

    let result = process_screener_until_stop(state, job.user_id, job.id, run_id).await;
    match result {
        Ok(JobTickOutcome::Continue) => {
            queue_job_if_still_running(state, job.id).await?;
        }
        Ok(JobTickOutcome::Completed { reason }) => {
            complete_job(state, job.id, run_id, &reason).await?;
        }
        Err(AppError::RateLimited {
            provider,
            retry_after,
        }) => {
            log_job_event(
                state,
                job.id,
                "rate_limited",
                &format!("{provider} quota exhausted until {retry_after}"),
                json!({ "provider": provider, "retry_after": retry_after }),
            )
            .await?;
            pause_rate_limited_job(state, job.id, &provider, retry_after).await?;
        }
        Err(AppError::JobBudgetExhausted {
            provider,
            used,
            limit,
        }) => {
            log_job_event(
                state,
                job.id,
                "job_budget_exhausted",
                &format!("{provider} job budget exhausted: {used}/{limit} credits"),
                json!({ "provider": provider, "used": used, "limit": limit }),
            )
            .await?;
            complete_job(
                state,
                job.id,
                run_id,
                &format!("{provider} job budget exhausted"),
            )
            .await?;
        }
        Err(err) => {
            log_job_event(state, job.id, "job_failed", &err.to_string(), json!({})).await?;
            fail_job(state, job.id, err.to_string()).await?;
        }
    }
    mirror_job_to_screener_run(state, job.id).await?;

    Ok(())
}

async fn process_screener_until_stop(
    state: &AppState,
    user_id: Uuid,
    job_id: Uuid,
    run_id: Uuid,
) -> Result<JobTickOutcome, AppError> {
    loop {
        if !job_is_running(state, job_id).await? {
            return Ok(JobTickOutcome::Continue);
        }

        match process_one_screener_item(state, user_id, job_id, run_id).await? {
            JobTickOutcome::Continue => continue,
            outcome => return Ok(outcome),
        }
    }
}

async fn discover_candidates(
    state: &AppState,
    user_id: Uuid,
    run_id: Uuid,
    excluded_tickers: &HashSet<String>,
) -> Result<Vec<EdgarTickerCompany>, AppError> {
    let recently_processed = recently_processed_tickers(state, user_id).await?;
    let companies = state.edgar.ticker_companies().await?;
    let mut name_matches = Vec::new();
    let mut profile_candidates = Vec::new();
    for company in companies {
        let ticker = company.ticker.trim();
        let normalized_ticker = ticker.replace('.', "-").to_uppercase();
        let eligible = !recently_processed.contains(&normalized_ticker)
            && !excluded_tickers.contains(&normalized_ticker)
            && is_screenable_ticker(ticker);
        if !eligible {
            continue;
        }

        if !theme_matches(&company.name, None).is_empty() {
            name_matches.push(company);
        } else {
            profile_candidates.push(company);
        }
    }

    let mut discovered = name_matches;
    for company in profile_candidates
        .into_iter()
        .take(MAX_DISCOVERY_PROFILE_LOOKUPS)
    {
        let cached_profile = state.edgar.has_profile_summary(&company.ticker).await;
        match state.edgar.company_profile_summary(&company).await {
            Ok(profile) => {
                if !theme_matches(&profile.name, profile.sic_description.as_deref()).is_empty() {
                    discovered.push(EdgarTickerCompany {
                        ticker: company.ticker,
                        name: profile.name,
                    });
                }
            }
            Err(err @ AppError::RateLimited { .. }) => return Err(err),
            Err(err) => {
                tracing::debug!(
                    ticker = %company.ticker,
                    error = %err,
                    "skipping SEC profile enrichment during discovery"
                );
            }
        }
        if !cached_profile {
            sleep(TokioDuration::from_millis(DISCOVERY_SEC_PROFILE_PAUSE_MS)).await;
        }
    }

    discovered.sort_by(|left, right| {
        randomized_candidate_score(run_id, right)
            .cmp(&randomized_candidate_score(run_id, left))
            .then_with(|| left.ticker.cmp(&right.ticker))
    });
    discovered.truncate(MAX_DISCOVERY_NAMES);

    Ok(discovered)
}

async fn process_one_screener_item(
    state: &AppState,
    user_id: Uuid,
    job_id: Uuid,
    run_id: Uuid,
) -> Result<JobTickOutcome, AppError> {
    ensure_screener_items(state, user_id, job_id, run_id).await?;

    let item = match next_screener_item(state, job_id).await? {
        Some(item) => item,
        None => {
            if enqueue_more_screener_items(state, user_id, job_id, run_id).await? == 0 {
                return Ok(JobTickOutcome::Completed {
                    reason: "no more discoverable tickers".to_string(),
                });
            }
            let Some(item) = next_screener_item(state, job_id).await? else {
                return Ok(JobTickOutcome::Completed {
                    reason: "no more discoverable tickers".to_string(),
                });
            };
            item
        }
    };

    sqlx::query(
        r#"
        update background_job_items
        set status = 'processing',
            attempts = attempts + 1,
            updated_at = now()
        where id = $1
        "#,
    )
    .bind(item.id)
    .execute(&state.db)
    .await?;

    let company = EdgarTickerCompany {
        ticker: item.item_key.clone(),
        name: item
            .payload
            .0
            .get("company_name")
            .and_then(|value| value.as_str())
            .unwrap_or(&item.item_key)
            .to_string(),
    };
    log_job_event(
        state,
        job_id,
        "ticker_started",
        &format!("Processing {}", company.ticker),
        json!({ "ticker": company.ticker, "company_name": company.name }),
    )
    .await?;
    let job_budget = JobBudget {
        job_id,
        credit_limit: screener_twelve_budget_limit(),
    };

    match screen_company(state, user_id, &company, Some(job_budget)).await {
        Ok(Some(result)) => {
            let ticker = result.ticker.clone();
            let status = result.status.clone();
            let rank = next_result_rank(state, run_id).await?;
            let result_id = insert_result(state, run_id, rank, result).await?;
            finish_item(state, item.id, "completed", Some(result_id), None).await?;
            log_job_event(
                state,
                job_id,
                "ticker_completed",
                &format!("{ticker} completed as {status}"),
                json!({ "ticker": ticker, "status": status, "result_id": result_id }),
            )
            .await?;
            sync_progress(state, job_id, run_id).await?;
            if matches!(status.as_str(), "Entry Candidate" | "Watch") {
                skip_remaining_items(state, job_id, &format!("candidate found: {ticker}")).await?;
                sync_progress(state, job_id, run_id).await?;
                log_job_event(
                    state,
                    job_id,
                    "candidate_found",
                    &format!("Candidate found: {ticker} ({status})"),
                    json!({ "ticker": ticker, "status": status }),
                )
                .await?;
                return Ok(JobTickOutcome::Completed {
                    reason: format!("candidate found: {ticker}"),
                });
            }
        }
        Ok(None) => {
            let rank = next_result_rank(state, run_id).await?;
            let result_id = insert_processed_marker(
                state,
                run_id,
                rank,
                &company,
                "screened out before full classification",
            )
            .await?;
            finish_item(state, item.id, "skipped", Some(result_id), None).await?;
            log_job_event(
                state,
                job_id,
                "ticker_skipped",
                &format!("{} skipped", company.ticker),
                json!({ "ticker": company.ticker, "result_id": result_id }),
            )
            .await?;
        }
        Err(err @ AppError::RateLimited { .. }) => {
            sqlx::query(
                r#"
                update background_job_items
                set status = 'queued',
                    updated_at = now()
                where id = $1
                "#,
            )
            .bind(item.id)
            .execute(&state.db)
            .await?;
            return Err(err);
        }
        Err(err @ AppError::JobBudgetExhausted { .. }) => {
            sqlx::query(
                r#"
                update background_job_items
                set status = 'queued',
                    updated_at = now()
                where id = $1
                "#,
            )
            .bind(item.id)
            .execute(&state.db)
            .await?;
            return Err(err);
        }
        Err(AppError::MarketDataNotFound(message)) => {
            let rank = next_result_rank(state, run_id).await?;
            let result_id =
                insert_processed_marker(state, run_id, rank, &company, &message).await?;
            finish_item(
                state,
                item.id,
                "skipped",
                Some(result_id),
                Some(message.clone()),
            )
            .await?;
            log_job_event(
                state,
                job_id,
                "ticker_skipped",
                &format!("{} skipped: {message}", company.ticker),
                json!({ "ticker": company.ticker, "reason": message, "result_id": result_id }),
            )
            .await?;
        }
        Err(err) => {
            let error = err.to_string();
            let rank = next_result_rank(state, run_id).await?;
            let result_id = insert_processed_marker(
                state,
                run_id,
                rank,
                &company,
                &format!("processing failed: {error}"),
            )
            .await?;
            finish_item(
                state,
                item.id,
                "failed",
                Some(result_id),
                Some(err.to_string()),
            )
            .await?;
            log_job_event(
                state,
                job_id,
                "ticker_failed",
                &format!("{} failed: {error}", company.ticker),
                json!({ "ticker": company.ticker, "error": error, "result_id": result_id }),
            )
            .await?;
        }
    }

    sync_progress(state, job_id, run_id).await?;

    if queued_item_count(state, job_id).await? == 0
        && enqueue_more_screener_items(state, user_id, job_id, run_id).await? == 0
    {
        Ok(JobTickOutcome::Completed {
            reason: "no more discoverable tickers".to_string(),
        })
    } else {
        Ok(JobTickOutcome::Continue)
    }
}

async fn ensure_screener_items(
    state: &AppState,
    user_id: Uuid,
    job_id: Uuid,
    run_id: Uuid,
) -> Result<(), AppError> {
    let existing =
        sqlx::query_scalar::<_, i64>("select count(*) from background_job_items where job_id = $1")
            .bind(job_id)
            .fetch_one(&state.db)
            .await?;
    if existing > 0 {
        return Ok(());
    }

    enqueue_more_screener_items(state, user_id, job_id, run_id).await?;
    Ok(())
}

async fn enqueue_more_screener_items(
    state: &AppState,
    user_id: Uuid,
    job_id: Uuid,
    run_id: Uuid,
) -> Result<i32, AppError> {
    let budget_remaining =
        screener_twelve_budget_limit().saturating_sub(twelve_job_budget_used(state, job_id).await?);
    if budget_remaining <= 0 {
        return Ok(0);
    }

    let excluded_tickers = job_item_tickers(state, job_id).await?;
    let mut universe = discover_candidates(state, user_id, run_id, &excluded_tickers).await?;
    let batch_limit = SCREENER_DISCOVERY_BATCH_SIZE.min(budget_remaining as usize);
    universe.truncate(batch_limit);

    let mut inserted = 0;
    for company in &universe {
        let result = sqlx::query(
            r#"
            insert into background_job_items (
              job_id,
              item_key,
              status,
              payload,
              updated_at
            )
            values ($1, $2, 'queued', $3, now())
            on conflict (job_id, item_key) do nothing
            "#,
        )
        .bind(job_id)
        .bind(normalize_ticker(&company.ticker)?)
        .bind(SqlxJson(json!({ "company_name": company.name })))
        .execute(&state.db)
        .await?;
        inserted += result.rows_affected() as i32;
    }

    if inserted > 0 {
        log_job_event(
            state,
            job_id,
            "ticker_batch_queued",
            &format!("Queued {inserted} more tickers"),
            json!({ "count": inserted }),
        )
        .await?;
    }

    let total_items =
        sqlx::query_scalar::<_, i64>("select count(*) from background_job_items where job_id = $1")
            .bind(job_id)
            .fetch_one(&state.db)
            .await?;

    sqlx::query(
        r#"
        update background_jobs
        set progress_total = $2,
            updated_at = now()
        where id = $1
        "#,
    )
    .bind(job_id)
    .bind(total_items as i32)
    .execute(&state.db)
    .await?;

    sqlx::query(
        r#"
        update ai_screener_runs
        set universe_count = $2,
            updated_at = now()
        where id = $1
        "#,
    )
    .bind(run_id)
    .bind(total_items as i32)
    .execute(&state.db)
    .await?;

    Ok(inserted)
}

async fn next_screener_item(
    state: &AppState,
    job_id: Uuid,
) -> Result<Option<ScreenerItemRow>, AppError> {
    sqlx::query_as::<_, ScreenerItemRow>(
        r#"
        select id, item_key, payload
        from background_job_items
        where job_id = $1
          and status = 'queued'
        order by created_at
        limit 1
        "#,
    )
    .bind(job_id)
    .fetch_optional(&state.db)
    .await
    .map_err(Into::into)
}

async fn finish_item(
    state: &AppState,
    item_id: Uuid,
    status: &str,
    result_ref: Option<Uuid>,
    last_error: Option<String>,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        update background_job_items
        set status = $2,
            result_ref = $3,
            last_error = $4,
            updated_at = now()
        where id = $1
        "#,
    )
    .bind(item_id)
    .bind(status)
    .bind(result_ref)
    .bind(last_error)
    .execute(&state.db)
    .await?;

    Ok(())
}

async fn queued_item_count(state: &AppState, job_id: Uuid) -> Result<i64, AppError> {
    sqlx::query_scalar(
        "select count(*) from background_job_items where job_id = $1 and status = 'queued'",
    )
    .bind(job_id)
    .fetch_one(&state.db)
    .await
    .map_err(Into::into)
}

async fn job_item_tickers(state: &AppState, job_id: Uuid) -> Result<HashSet<String>, AppError> {
    let tickers = sqlx::query_scalar::<_, String>(
        "select item_key from background_job_items where job_id = $1",
    )
    .bind(job_id)
    .fetch_all(&state.db)
    .await?;

    Ok(tickers.into_iter().collect())
}

async fn twelve_job_budget_used(state: &AppState, job_id: Uuid) -> Result<i32, AppError> {
    let used = sqlx::query_scalar::<_, Option<i32>>(
        r#"
        select credit_count
        from background_job_provider_usage
        where job_id = $1
          and provider = 'twelvedata'
        "#,
    )
    .bind(job_id)
    .fetch_optional(&state.db)
    .await?
    .flatten()
    .unwrap_or(0);

    Ok(used)
}

async fn skip_remaining_items(
    state: &AppState,
    job_id: Uuid,
    reason: &str,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        update background_job_items
        set status = 'skipped',
            last_error = $2,
            updated_at = now()
        where job_id = $1
          and status = 'queued'
        "#,
    )
    .bind(job_id)
    .bind(reason)
    .execute(&state.db)
    .await?;
    Ok(())
}

async fn job_is_running(state: &AppState, job_id: Uuid) -> Result<bool, AppError> {
    let status =
        sqlx::query_scalar::<_, String>("select status from background_jobs where id = $1")
            .bind(job_id)
            .fetch_optional(&state.db)
            .await?;

    Ok(status.as_deref() == Some("running"))
}

async fn log_job_event(
    state: &AppState,
    job_id: Uuid,
    event_type: &str,
    message: &str,
    payload: Value,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        insert into background_job_events (
          job_id,
          event_type,
          message,
          payload
        )
        values ($1, $2, $3, $4)
        "#,
    )
    .bind(job_id)
    .bind(event_type)
    .bind(message)
    .bind(SqlxJson(payload))
    .execute(&state.db)
    .await?;
    Ok(())
}

fn screener_twelve_budget_limit() -> i32 {
    (TWELVE_DAILY_CREDIT_LIMIT * SCREENER_TWELVE_BUDGET_PERCENT) / 100
}

async fn next_result_rank(state: &AppState, run_id: Uuid) -> Result<i32, AppError> {
    let count =
        sqlx::query_scalar::<_, i64>("select count(*) from ai_screener_results where run_id = $1")
            .bind(run_id)
            .fetch_one(&state.db)
            .await?;
    Ok(count as i32 + 1)
}

async fn sync_progress(state: &AppState, job_id: Uuid, run_id: Uuid) -> Result<(), AppError> {
    let progress_current = sqlx::query_scalar::<_, i64>(
        r#"
        select count(*)
        from background_job_items
        where job_id = $1
          and status in ('completed', 'skipped', 'failed')
        "#,
    )
    .bind(job_id)
    .fetch_one(&state.db)
    .await? as i32;
    let result_count =
        sqlx::query_scalar::<_, i64>("select count(*) from ai_screener_results where run_id = $1")
            .bind(run_id)
            .fetch_one(&state.db)
            .await? as i32;

    sqlx::query(
        r#"
        update background_jobs
        set progress_current = $2,
            updated_at = now()
        where id = $1
        "#,
    )
    .bind(job_id)
    .bind(progress_current)
    .execute(&state.db)
    .await?;

    sqlx::query(
        r#"
        update ai_screener_runs
        set processed_count = $2,
            result_count = $3,
            updated_at = now()
        where id = $1
        "#,
    )
    .bind(run_id)
    .bind(progress_current)
    .bind(result_count)
    .execute(&state.db)
    .await?;

    Ok(())
}

async fn queue_job_if_still_running(state: &AppState, job_id: Uuid) -> Result<(), AppError> {
    sqlx::query(
        r#"
        update background_jobs
        set status = 'queued',
            run_after = now(),
            updated_at = now()
        where id = $1
          and status = 'running'
        "#,
    )
    .bind(job_id)
    .execute(&state.db)
    .await?;
    Ok(())
}

async fn pause_rate_limited_job(
    state: &AppState,
    job_id: Uuid,
    provider: &str,
    retry_after: DateTime<Utc>,
) -> Result<(), AppError> {
    let reason = format!("{provider} quota exhausted");
    sqlx::query(
        r#"
        update background_jobs
        set status = 'waiting_rate_limit',
            run_after = $2,
            status_reason = $3,
            updated_at = now()
        where id = $1
          and status = 'running'
        "#,
    )
    .bind(job_id)
    .bind(retry_after)
    .bind(reason)
    .execute(&state.db)
    .await?;
    Ok(())
}

async fn complete_job(
    state: &AppState,
    job_id: Uuid,
    run_id: Uuid,
    reason: &str,
) -> Result<(), AppError> {
    if reason.contains("budget exhausted") {
        skip_remaining_items(state, job_id, reason).await?;
    }
    sync_progress(state, job_id, run_id).await?;
    sqlx::query(
        r#"
        update background_jobs
        set status = 'completed',
            status_reason = $2,
            completed_at = now(),
            updated_at = now()
        where id = $1
          and status = 'running'
        "#,
    )
    .bind(job_id)
    .bind(reason)
    .execute(&state.db)
    .await?;
    log_job_event(
        state,
        job_id,
        "job_completed",
        &format!("Screener job completed: {reason}"),
        json!({ "reason": reason }),
    )
    .await?;
    Ok(())
}

async fn fail_job(state: &AppState, job_id: Uuid, error: String) -> Result<(), AppError> {
    sqlx::query(
        r#"
        update background_jobs
        set status = 'failed',
            error = $2,
            failed_at = now(),
            updated_at = now()
        where id = $1
          and status = 'running'
        "#,
    )
    .bind(job_id)
    .bind(error)
    .execute(&state.db)
    .await?;
    Ok(())
}

async fn mirror_job_to_screener_run(state: &AppState, job_id: Uuid) -> Result<(), AppError> {
    sqlx::query(
        r#"
        update ai_screener_runs r
        set status = j.status,
            run_after = j.run_after,
            status_reason = j.status_reason,
            error = j.error,
            started_at = coalesce(r.started_at, j.started_at),
            completed_at = case when j.status = 'completed' then j.completed_at else r.completed_at end,
            updated_at = now()
        from background_jobs j
        where r.job_id = j.id
          and j.id = $1
        "#,
    )
    .bind(job_id)
    .execute(&state.db)
    .await?;
    Ok(())
}

async fn recover_interrupted_screener_jobs(state: &AppState) -> Result<(), AppError> {
    sqlx::query(
        r#"
        update background_job_items
        set status = 'queued',
            updated_at = now()
        where status = 'processing'
        "#,
    )
    .execute(&state.db)
    .await?;

    sqlx::query(
        r#"
        update background_jobs
        set status = 'queued',
            run_after = now(),
            updated_at = now()
        where job_type = $1
          and status = 'running'
        "#,
    )
    .bind(SCREENER_JOB_TYPE)
    .execute(&state.db)
    .await?;

    Ok(())
}

async fn recover_user_screener_jobs(state: &AppState, user_id: Uuid) -> Result<(), AppError> {
    recover_interrupted_screener_jobs(state).await?;
    let job_ids = sqlx::query_scalar::<_, Uuid>(
        r#"
        select id
        from background_jobs
        where user_id = $1
          and job_type = $2
          and status in ('queued', 'running', 'waiting_rate_limit', 'stopped')
        "#,
    )
    .bind(user_id)
    .bind(SCREENER_JOB_TYPE)
    .fetch_all(&state.db)
    .await?;

    for job_id in job_ids {
        mirror_job_to_screener_run(state, job_id).await?;
    }

    Ok(())
}

async fn resume_job_if_paused(state: &AppState, run_id: Uuid) -> Result<(), AppError> {
    let job_id =
        sqlx::query_scalar::<_, Option<Uuid>>("select job_id from ai_screener_runs where id = $1")
            .bind(run_id)
            .fetch_optional(&state.db)
            .await?
            .flatten();
    let Some(job_id) = job_id else {
        return Ok(());
    };

    sqlx::query(
        r#"
        update background_jobs
        set status = 'queued',
            run_after = now(),
            status_reason = null,
            error = null,
            stopped_at = null,
            updated_at = now()
        where id = $1
          and status in ('waiting_rate_limit', 'stopped', 'failed')
        "#,
    )
    .bind(job_id)
    .execute(&state.db)
    .await?;
    mirror_job_to_screener_run(state, job_id).await?;
    Ok(())
}

fn job_run_id(payload: &Value) -> Result<Uuid, AppError> {
    let value = payload
        .get("run_id")
        .and_then(|value| value.as_str())
        .ok_or_else(|| AppError::BadRequest("screener job is missing run_id".to_string()))?;
    Uuid::parse_str(value).map_err(|err| AppError::BadRequest(err.to_string()))
}

async fn recently_processed_tickers(
    state: &AppState,
    user_id: Uuid,
) -> Result<HashSet<String>, AppError> {
    let tickers = sqlx::query_scalar(
        r#"
        select distinct r.ticker
        from ai_screener_results r
        join ai_screener_runs run on run.id = r.run_id
        where run.user_id = $1
          and r.created_at > now() - interval '24 hours'
        "#,
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;

    Ok(tickers.into_iter().collect())
}

async fn screen_company(
    state: &AppState,
    user_id: Uuid,
    company: &EdgarTickerCompany,
    job_budget: Option<JobBudget>,
) -> Result<Option<InternalResult>, AppError> {
    let ticker = normalize_ticker(&company.ticker)?;
    let prices = cached_or_refreshed_daily_prices(state, &ticker, job_budget).await?;
    let technical = technical_snapshot(&prices);
    let profile = match state.edgar.company_profile(&ticker).await {
        Ok(profile) => Some(profile),
        Err(err @ AppError::RateLimited { .. }) => return Err(err),
        Err(_) => None,
    };
    let sic_description = profile
        .as_ref()
        .and_then(|profile| profile.sic_description.clone());
    let company_name = profile
        .as_ref()
        .map(|profile| profile.name.clone())
        .unwrap_or_else(|| company.name.clone());
    let themes = theme_matches(&company_name, sic_description.as_deref());
    let deterministic = deterministic_ai_score(&company_name, sic_description.as_deref(), &themes);
    let ai = enrich_ai(
        state,
        &ticker,
        &company_name,
        sic_description.as_deref(),
        &themes,
        &deterministic,
    )
    .await;
    let mut ai_tier = ai
        .as_ref()
        .and_then(|ai| normalize_tier(ai.tier.as_deref()))
        .or_else(|| deterministic_tier(deterministic.score));
    let mut ai_score = clamp_score(
        deterministic.score
            + ai.as_ref()
                .map(|ai| ai.score_adjustment.clamp(-15, 15))
                .unwrap_or(0),
    );
    let manual = fetch_override(state, user_id, &ticker).await?;
    if let Some(manual_tier) = manual.manual_ai_tier.clone() {
        ai_tier = Some(manual_tier);
    }
    if let Some(manual_score) = manual.manual_ai_score {
        ai_score = manual_score;
    }

    let mut classification = classify_candidate(ai_score, technical.as_ref());
    if let Some(manual_status) = manual.manual_status.clone() {
        classification.status = manual_status;
        classification.rejection_reason = if classification.status == "Rejected" {
            classification
                .rejection_reason
                .or_else(|| Some("manual status override".to_string()))
        } else {
            None
        };
        classification.explanation = "manual status override applied".to_string();
    }
    let rationale = json!({
        "company_summary": company_summary(&company_name, sic_description.as_deref()),
        "ai": {
            "deterministic_score": deterministic.score,
            "theme_matches": themes,
            "provider": if state.openai.is_some() { "openai" } else { "deterministic" },
            "enrichment_available": ai.is_some(),
            "reasons": ai.as_ref().map(|ai| ai.ai_relevance_reasons.clone()).unwrap_or_else(|| deterministic.reasons.clone()),
            "warnings": ai.as_ref().map(|ai| ai.warnings.clone()).unwrap_or_default(),
            "confidence": ai.as_ref().map(|ai| ai.confidence).unwrap_or(0.0)
        },
        "technical": classification.explanation,
        "technical_metrics": technical.as_ref().map(|tech| json!({
            "recovery_from_low": tech.recovery_from_low,
            "days_since_low": tech.days_since_low
        })),
        "manual_override_applied": manual.manual_ai_tier.is_some() || manual.manual_ai_score.is_some() || manual.manual_status.is_some()
    });

    Ok(Some(InternalResult {
        ticker,
        company_name,
        ai_tier,
        ai_score,
        status: classification.status,
        current_price: technical.as_ref().map(|tech| tech.current_price),
        correction_depth: technical.as_ref().map(|tech| tech.correction_depth),
        trend_distance: technical.as_ref().map(|tech| tech.trend_distance),
        momentum_condition: classification.momentum_condition,
        volume_condition: classification.volume_condition,
        rejection_reason: classification.rejection_reason,
        rationale,
    }))
}

async fn enrich_ai(
    state: &AppState,
    ticker: &str,
    name: &str,
    sic_description: Option<&str>,
    themes: &[String],
    deterministic: &DeterministicAiScore,
) -> Option<AiEnrichment> {
    let client = state.openai.as_ref()?;
    client
        .enrich_ai_relevance(&AiCompanyContext {
            ticker: ticker.to_string(),
            name: name.to_string(),
            sic_description: sic_description.map(str::to_string),
            deterministic_tier: deterministic_tier(deterministic.score),
            deterministic_score: deterministic.score,
            theme_matches: themes.to_vec(),
        })
        .await
        .ok()
}

fn classify_candidate(ai_score: i32, technical: Option<&TechnicalSnapshot>) -> Classification {
    let Some(tech) = technical else {
        return Classification::rejected("insufficient price history", "Unknown", "Unknown");
    };

    let volume_condition = if tech.avg_volume >= 1_000_000.0 {
        "Strong"
    } else if tech.avg_volume >= 200_000.0 {
        "Acceptable"
    } else {
        "Weak"
    };
    let momentum_condition = if tech.rebound_signal {
        "Recovering"
    } else if tech.correction_depth >= 7.0 {
        "Pullback"
    } else if tech.close_above_ma20 {
        "Firm"
    } else {
        "Soft"
    };

    if tech.current_price < 5.0 {
        return Classification::rejected("price is below $5", momentum_condition, volume_condition);
    }
    if tech.avg_volume < 200_000.0 {
        return Classification::rejected(
            "average volume is below 200k shares",
            momentum_condition,
            volume_condition,
        );
    }
    if !tech.long_trend_healthy {
        return Classification::rejected(
            "long-term trend is not healthy",
            momentum_condition,
            volume_condition,
        );
    }
    if tech.correction_depth > 35.0 {
        return Classification::rejected(
            "correction is too deep",
            momentum_condition,
            volume_condition,
        );
    }
    if ai_score < 40 {
        return Classification {
            status: "Ignore".to_string(),
            momentum_condition: momentum_condition.to_string(),
            volume_condition: volume_condition.to_string(),
            rejection_reason: None,
            explanation: "AI relevance score is below the review threshold".to_string(),
        };
    }
    if tech.recovery_from_low > 16.0 && tech.correction_depth < 14.0 {
        return Classification {
            status: "Ignore".to_string(),
            momentum_condition: momentum_condition.to_string(),
            volume_condition: volume_condition.to_string(),
            rejection_reason: None,
            explanation: format!(
                "pullback has already recovered {:.1}% from the local correction low",
                tech.recovery_from_low
            ),
        };
    }
    if (8.0..=24.0).contains(&tech.correction_depth)
        && tech.rebound_signal
        && tech.recovery_from_low <= 12.0
        && tech.days_since_low <= 12
    {
        return Classification {
            status: "Entry Candidate".to_string(),
            momentum_condition: momentum_condition.to_string(),
            volume_condition: volume_condition.to_string(),
            rejection_reason: None,
            explanation: format!(
                "healthy trend, controlled pullback, recent recovery signal, and {:.1}% recovery from the local correction low",
                tech.recovery_from_low
            ),
        };
    }
    if (8.0..=30.0).contains(&tech.correction_depth) && tech.recovery_from_low <= 18.0 {
        return Classification {
            status: "Watch".to_string(),
            momentum_condition: momentum_condition.to_string(),
            volume_condition: volume_condition.to_string(),
            rejection_reason: None,
            explanation: format!(
                "healthy trend with a local correction inside the accepted pullback range and {:.1}% recovery from the local low",
                tech.recovery_from_low
            ),
        };
    }

    Classification {
        status: "Ignore".to_string(),
        momentum_condition: momentum_condition.to_string(),
        volume_condition: volume_condition.to_string(),
        rejection_reason: None,
        explanation: "ticker is AI-relevant but not currently in the target correction zone"
            .to_string(),
    }
}

fn technical_snapshot(prices: &[PricePoint]) -> Option<TechnicalSnapshot> {
    if prices.len() < 80 {
        return None;
    }

    let current = prices.last()?.close;
    let recent_window_start = prices.len().saturating_sub(60);
    let recent_window = &prices[recent_window_start..];
    let (recent_high_offset, recent_high) =
        recent_window
            .iter()
            .enumerate()
            .fold((0usize, 0.0f64), |best, (index, price)| {
                if price.high > best.1 {
                    (index, price.high)
                } else {
                    best
                }
            });
    let high_index = recent_window_start + recent_high_offset;
    let after_high = &prices[high_index..];
    let (low_after_high_offset, correction_low) =
        after_high
            .iter()
            .enumerate()
            .fold((0usize, f64::MAX), |best, (index, price)| {
                if price.low < best.1 {
                    (index, price.low)
                } else {
                    best
                }
            });
    let low_index = high_index + low_after_high_offset;
    let ma20 = average_close(prices, 20)?;
    let ma50 = average_close(prices, 50)?;
    let ma150 = average_close(prices, 150).or_else(|| average_close(prices, prices.len()))?;
    let avg_volume = average_volume(prices, 30);
    let ma5 = average_close(prices, 5)?;
    let prior_ma5 = if prices.len() > 6 {
        average_close(&prices[..prices.len() - 1], 5).unwrap_or(ma5)
    } else {
        ma5
    };
    let previous_close = prices
        .get(prices.len().saturating_sub(2))
        .map(|price| price.close)
        .unwrap_or(current);
    let correction_depth = if recent_high <= 0.0 {
        0.0
    } else {
        ((recent_high - current) / recent_high) * 100.0
    };
    let trend_distance = ((current - ma150) / ma150) * 100.0;
    let recovery_from_low = if correction_low <= 0.0 || correction_low == f64::MAX {
        0.0
    } else {
        ((current - correction_low) / correction_low) * 100.0
    };
    let days_since_low = prices.len().saturating_sub(low_index + 1);

    Some(TechnicalSnapshot {
        current_price: current,
        correction_depth,
        trend_distance,
        recovery_from_low,
        days_since_low,
        avg_volume,
        long_trend_healthy: current >= ma150 * 0.94
            && ma50 >= ma150 * 0.96
            && trend_distance > -8.0,
        rebound_signal: current > previous_close
            && current >= ma5
            && ma5 >= prior_ma5
            && current >= ma20 * 0.96,
        close_above_ma20: current >= ma20,
    })
}

fn deterministic_ai_score(
    name: &str,
    sic_description: Option<&str>,
    themes: &[String],
) -> DeterministicAiScore {
    let mut score = 20 + (themes.len() as i32 * 8).min(32);
    let haystack = format!("{} {}", name, sic_description.unwrap_or("")).to_lowercase();
    let mut reasons = Vec::new();

    if contains_any(
        &haystack,
        &[
            "semiconductor",
            "chip",
            "micro device",
            "accelerated",
            "electronic component",
        ],
    ) {
        score += 25;
        reasons.push("semiconductor or accelerated computing exposure".to_string());
    }
    if contains_any(
        &haystack,
        &[
            "cloud",
            "data center",
            "digital infrastructure",
            "network",
            "optical",
            "server",
        ],
    ) {
        score += 18;
        reasons
            .push("cloud, data center, networking, or server infrastructure exposure".to_string());
    }
    if contains_any(
        &haystack,
        &[
            "electric",
            "power",
            "power distribution",
            "switchgear",
            "busbar",
            "ups",
            "cooling",
            "air-cond",
            "hvac",
            "refrig",
            "thermal management",
            "automation",
            "test equipment",
        ],
    ) {
        score += 12;
        reasons.push("physical infrastructure exposure linked to data center buildout".to_string());
    }
    if contains_any(
        &haystack,
        &["software", "design automation", "eda", "analytics"],
    ) {
        score += 10;
        reasons.push("software or design tooling exposure".to_string());
    }
    if reasons.is_empty() {
        reasons.push("matched broad AI infrastructure discovery keywords".to_string());
    }

    DeterministicAiScore {
        score: clamp_score(score),
        reasons,
    }
}

fn theme_matches(name: &str, sic_description: Option<&str>) -> Vec<String> {
    let haystack = format!("{} {}", name, sic_description.unwrap_or("")).to_lowercase();
    let mut matches = Vec::new();
    for (label, keywords) in [
        (
            "chips",
            &[
                "semiconductor",
                "chip",
                "micro device",
                "integrated circuit",
                "processor",
                "electronic component",
            ][..],
        ),
        (
            "cloud-data-center",
            &[
                "cloud",
                "data center",
                "digital infrastructure",
                "server",
                "super computer",
                "computing",
                "rack",
            ][..],
        ),
        (
            "networking-optical",
            &[
                "network",
                "optical",
                "interconnect",
                "communications equipment",
            ][..],
        ),
        (
            "power-cooling",
            &[
                "electric",
                "power",
                "power distribution",
                "switchgear",
                "busbar",
                "pdu",
                "ups",
                "cooling",
                "air-cond",
                "hvac",
                "refrig",
                "thermal",
                "thermal management",
                "energy",
            ][..],
        ),
        (
            "automation-testing",
            &[
                "automation",
                "test equipment",
                "measurement",
                "electronic instruments",
                "electronic components",
            ][..],
        ),
        (
            "software-eda",
            &[
                "software",
                "design automation",
                "eda",
                "analytics",
                "cadence",
                "synopsys",
            ][..],
        ),
    ] {
        if contains_any(&haystack, keywords) {
            matches.push(label.to_string());
        }
    }

    matches
}

#[cfg(test)]
mod screener_tests {
    use super::*;

    #[test]
    fn vrt_sec_profile_matches_infrastructure_theme() {
        let matches = theme_matches("Vertiv Holdings Co", Some("Electronic Components, NEC"));

        assert!(!matches.is_empty());
    }

    #[test]
    fn hvac_sec_profiles_match_cooling_theme() {
        let jci_matches = theme_matches(
            "Johnson Controls International plc",
            Some("Air-Cond & Warm Air Heatg Equip & Comm & Indl Refrig Equip"),
        );
        let tt_matches = theme_matches(
            "Trane Technologies plc",
            Some("Auto Controls For Regulating Residential & Comml Environments"),
        );

        assert!(jci_matches.contains(&"power-cooling".to_string()));
        assert!(tt_matches.is_empty());
    }
}

fn candidate_name_score(name: &str) -> i32 {
    let themes = theme_matches(name, None);
    deterministic_ai_score(name, None, &themes).score
}

fn randomized_candidate_score(run_id: Uuid, company: &EdgarTickerCompany) -> i32 {
    candidate_name_score(&company.name) * 10 + run_random_bucket(run_id, &company.ticker)
}

fn run_random_bucket(run_id: Uuid, ticker: &str) -> i32 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in run_id
        .as_bytes()
        .iter()
        .copied()
        .chain(ticker.as_bytes().iter().copied())
    {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }

    (hash % 100) as i32
}

async fn cached_or_refreshed_daily_prices(
    state: &AppState,
    ticker: &str,
    job_budget: Option<JobBudget>,
) -> Result<Vec<PricePoint>, AppError> {
    let start_date = Utc::now().date_naive() - Duration::days(380);
    let stats = sqlx::query_as::<_, DailyPriceCacheStats>(
        r#"
        select
          count(*)::bigint as count,
          min(date) as earliest_date,
          max(date) as latest_date,
          max(fetched_at) as recent_fetched_at
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
    let stale = stats
        .recent_fetched_at
        .map(|fetched_at| Utc::now() - fetched_at > Duration::hours(12))
        .unwrap_or(true);
    let incomplete = stats.count < 80
        || stats
            .earliest_date
            .map(|date| date > start_date + Duration::days(20))
            .unwrap_or(true)
        || stats
            .latest_date
            .map(|date| date < today - Duration::days(7))
            .unwrap_or(true);

    if incomplete || stale {
        let prices = state
            .market_data
            .daily_price_history_with_budget(ticker, PRICE_OUTPUT_SIZE, job_budget)
            .await?;
        upsert_daily_prices(state, ticker, &prices).await?;
    }

    let rows = sqlx::query_as::<_, DailyPriceRow>(
        r#"
        select date, open, high, low, close, volume
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
    .await?;

    Ok(rows.into_iter().map(Into::into).collect())
}

async fn upsert_daily_prices(
    state: &AppState,
    ticker: &str,
    prices: &[PricePoint],
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

async fn insert_result(
    state: &AppState,
    run_id: Uuid,
    rank: i32,
    result: InternalResult,
) -> Result<Uuid, AppError> {
    let row_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        insert into ai_screener_results (
          run_id,
          ticker,
          company_name,
          ai_tier,
          ai_score,
          status,
          current_price,
          correction_depth,
          trend_distance,
          momentum_condition,
          volume_condition,
          rejection_reason,
          rationale,
          rank
        )
        values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
        returning id
        "#,
    )
    .bind(run_id)
    .bind(result.ticker)
    .bind(result.company_name)
    .bind(result.ai_tier)
    .bind(result.ai_score)
    .bind(result.status)
    .bind(result.current_price)
    .bind(result.correction_depth)
    .bind(result.trend_distance)
    .bind(result.momentum_condition)
    .bind(result.volume_condition)
    .bind(result.rejection_reason)
    .bind(SqlxJson(result.rationale))
    .bind(rank)
    .fetch_one(&state.db)
    .await?;

    Ok(row_id)
}

async fn insert_processed_marker(
    state: &AppState,
    run_id: Uuid,
    rank: i32,
    company: &EdgarTickerCompany,
    reason: &str,
) -> Result<Uuid, AppError> {
    insert_result(
        state,
        run_id,
        rank,
        InternalResult {
            ticker: normalize_ticker(&company.ticker)?,
            company_name: company.name.clone(),
            ai_tier: None,
            ai_score: 0,
            status: "Rejected".to_string(),
            current_price: None,
            correction_depth: None,
            trend_distance: None,
            momentum_condition: "Unknown".to_string(),
            volume_condition: "Unknown".to_string(),
            rejection_reason: Some(reason.to_string()),
            rationale: json!({
                "company_summary": company.name,
                "processing": {
                    "record_type": "skipped",
                    "reason": reason
                }
            }),
        },
    )
    .await
}

async fn read_run_by_id(
    state: &AppState,
    user_id: Uuid,
    run_id: Uuid,
) -> Result<AiScreenerRun, AppError> {
    let row = sqlx::query_as::<_, RunRow>(
        r#"
        select
          id,
          job_id,
          status,
          run_after,
          status_reason,
          universe_count,
          processed_count,
          result_count,
          error,
          started_at,
          completed_at,
          created_at,
          updated_at
        from ai_screener_runs
        where id = $1
          and user_id = $2
        "#,
    )
    .bind(run_id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("screener run not found".to_string()))?;

    let results = sqlx::query_as::<_, ResultRow>(
        r#"
        select
          r.id,
          r.run_id,
          run.completed_at as run_completed_at,
          r.ticker,
          r.company_name,
          coalesce(o.manual_ai_tier, r.ai_tier) as ai_tier,
          coalesce(o.manual_ai_score, r.ai_score) as ai_score,
          coalesce(o.manual_status, r.status) as status,
          r.current_price,
          r.correction_depth,
          r.trend_distance,
          r.momentum_condition,
          r.volume_condition,
          case
            when o.manual_status = 'Rejected' then coalesce(r.rejection_reason, 'manual status override')
            when o.manual_status is not null then null
            else r.rejection_reason
          end as rejection_reason,
          r.rationale,
          r.rank,
          o.manual_ai_tier,
          o.manual_ai_score,
          o.manual_status,
          coalesce(o.notes, '') as manual_notes,
          r.created_at as processed_at
        from ai_screener_results r
        join ai_screener_runs run on run.id = r.run_id
        left join ai_screener_overrides o
          on o.user_id = $1
         and o.ticker = r.ticker
        where run.user_id = $1
        order by coalesce(run.completed_at, run.created_at) desc, r.created_at desc, r.ticker
        "#,
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(Into::into)
    .collect();
    let events = if let Some(job_id) = row.job_id {
        sqlx::query_as::<_, EventRow>(
            r#"
            select id, event_type, message, payload, created_at
            from background_job_events
            where job_id = $1
            order by created_at desc
            limit 50
            "#,
        )
        .bind(job_id)
        .fetch_all(&state.db)
        .await?
        .into_iter()
        .map(Into::into)
        .collect::<Vec<BackgroundJobEvent>>()
    } else {
        Vec::new()
    };
    let latest_event = events.first().cloned();
    let twelve_budget_used = if let Some(job_id) = row.job_id {
        sqlx::query_scalar::<_, Option<i32>>(
            r#"
            select credit_count
            from background_job_provider_usage
            where job_id = $1
              and provider = 'twelvedata'
            "#,
        )
        .bind(job_id)
        .fetch_optional(&state.db)
        .await?
        .flatten()
        .unwrap_or(0)
    } else {
        0
    };

    Ok(AiScreenerRun {
        id: row.id,
        job_id: row.job_id,
        status: row.status,
        run_after: row.run_after,
        status_reason: row.status_reason,
        universe_count: row.universe_count,
        processed_count: row.processed_count,
        result_count: row.result_count,
        error: row.error,
        started_at: row.started_at,
        completed_at: row.completed_at,
        created_at: row.created_at,
        updated_at: row.updated_at,
        results,
        events,
        latest_event,
        twelve_budget_used,
        twelve_budget_limit: screener_twelve_budget_limit(),
    })
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

async fn latest_run_id(state: &AppState, user_id: Uuid) -> Result<Option<Uuid>, AppError> {
    sqlx::query_scalar(
        r#"
        select id
        from ai_screener_runs
        where user_id = $1
        order by created_at desc
        limit 1
        "#,
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await
    .map_err(Into::into)
}

async fn active_run(state: &AppState, user_id: Uuid) -> Result<Option<Uuid>, AppError> {
    sqlx::query_scalar(
        r#"
        select id
        from ai_screener_runs
        where user_id = $1
          and status in ('queued', 'running', 'waiting_rate_limit', 'stopped')
        order by created_at desc
        limit 1
        "#,
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await
    .map_err(Into::into)
}

async fn fetch_override(
    state: &AppState,
    user_id: Uuid,
    ticker: &str,
) -> Result<OverrideRow, AppError> {
    Ok(sqlx::query_as::<_, OverrideRow>(
        r#"
        select manual_ai_tier, manual_ai_score, manual_status
        from ai_screener_overrides
        where user_id = $1
          and ticker = $2
        "#,
    )
    .bind(user_id)
    .bind(ticker)
    .fetch_optional(&state.db)
    .await?
    .unwrap_or_default())
}

fn validate_override(payload: &UpdateAiScreenerOverride) -> Result<(), AppError> {
    if payload
        .manual_ai_tier
        .as_deref()
        .is_some_and(|tier| normalize_tier(Some(tier)).is_none())
    {
        return Err(AppError::BadRequest(
            "manual_ai_tier must be 1, 2, 3, or null".to_string(),
        ));
    }
    if payload
        .manual_ai_score
        .is_some_and(|score| !(0..=100).contains(&score))
    {
        return Err(AppError::BadRequest(
            "manual_ai_score must be between 0 and 100".to_string(),
        ));
    }
    if payload
        .manual_status
        .as_deref()
        .is_some_and(|status| !is_valid_screener_status(status))
    {
        return Err(AppError::BadRequest(
            "manual_status must be Ignore, Watch, Entry Candidate, Rejected, or null".to_string(),
        ));
    }

    Ok(())
}

fn is_valid_screener_status(status: &str) -> bool {
    matches!(status, "Ignore" | "Watch" | "Entry Candidate" | "Rejected")
}

fn normalize_ticker(value: &str) -> Result<String, AppError> {
    let ticker = value.trim().replace('.', "-").to_uppercase();
    if ticker.is_empty() {
        return Err(AppError::BadRequest("ticker is required".to_string()));
    }
    Ok(ticker)
}

fn is_screenable_ticker(value: &str) -> bool {
    let ticker = value.trim();
    let normalized = ticker.replace('.', "-").to_uppercase();
    !ticker.is_empty()
        && ticker.len() <= 7
        && ticker
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '.')
        && !normalized.ends_with("-WT")
        && !normalized.ends_with("-WS")
        && !normalized.ends_with("-W")
        && !normalized.ends_with("-U")
        && !normalized.ends_with("-RT")
        && !normalized.ends_with("WS")
        && !normalized.ends_with("W")
        && !normalized.ends_with("Y")
        && !normalized.ends_with("F")
        && !normalized.ends_with("RT")
}

fn normalize_tier(value: Option<&str>) -> Option<String> {
    match value?.trim() {
        "1" | "Tier 1" | "tier 1" => Some("1".to_string()),
        "2" | "Tier 2" | "tier 2" => Some("2".to_string()),
        "3" | "Tier 3" | "tier 3" => Some("3".to_string()),
        _ => None,
    }
}

fn deterministic_tier(score: i32) -> Option<String> {
    if score >= 75 {
        Some("1".to_string())
    } else if score >= 58 {
        Some("2".to_string())
    } else if score >= 40 {
        Some("3".to_string())
    } else {
        None
    }
}

fn clamp_score(value: i32) -> i32 {
    value.clamp(0, 100)
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

fn average_close(prices: &[PricePoint], length: usize) -> Option<f64> {
    if prices.len() < length || length == 0 {
        return None;
    }
    let sum = prices
        .iter()
        .rev()
        .take(length)
        .map(|price| price.close)
        .sum::<f64>();
    Some(sum / length as f64)
}

fn average_volume(prices: &[PricePoint], length: usize) -> f64 {
    let values: Vec<u64> = prices
        .iter()
        .rev()
        .take(length)
        .filter_map(|price| price.volume)
        .collect();
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<u64>() as f64 / values.len() as f64
}

fn company_summary(name: &str, sic_description: Option<&str>) -> String {
    match sic_description {
        Some(sic) => format!("{name} is classified by SEC under {sic}."),
        None => format!("{name} matched broad AI infrastructure discovery terms."),
    }
}

fn f64_cents(value: f64) -> i64 {
    (value * 100.0).round() as i64
}

fn cents_f64(value: i64) -> f64 {
    value as f64 / 100.0
}

#[derive(Debug, Clone)]
struct InternalResult {
    ticker: String,
    company_name: String,
    ai_tier: Option<String>,
    ai_score: i32,
    status: String,
    current_price: Option<f64>,
    correction_depth: Option<f64>,
    trend_distance: Option<f64>,
    momentum_condition: String,
    volume_condition: String,
    rejection_reason: Option<String>,
    rationale: Value,
}

#[derive(Debug, Clone, Copy)]
struct TechnicalSnapshot {
    current_price: f64,
    correction_depth: f64,
    trend_distance: f64,
    recovery_from_low: f64,
    days_since_low: usize,
    avg_volume: f64,
    long_trend_healthy: bool,
    rebound_signal: bool,
    close_above_ma20: bool,
}

#[derive(Debug, Clone)]
struct Classification {
    status: String,
    momentum_condition: String,
    volume_condition: String,
    rejection_reason: Option<String>,
    explanation: String,
}

impl Classification {
    fn rejected(reason: &str, momentum_condition: &str, volume_condition: &str) -> Self {
        Self {
            status: "Rejected".to_string(),
            momentum_condition: momentum_condition.to_string(),
            volume_condition: volume_condition.to_string(),
            rejection_reason: Some(reason.to_string()),
            explanation: reason.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
struct DeterministicAiScore {
    score: i32,
    reasons: Vec<String>,
}

#[derive(Default, sqlx::FromRow)]
struct OverrideRow {
    manual_ai_tier: Option<String>,
    manual_ai_score: Option<i32>,
    manual_status: Option<String>,
}

#[derive(sqlx::FromRow)]
struct RunRow {
    id: Uuid,
    job_id: Option<Uuid>,
    status: String,
    run_after: Option<DateTime<Utc>>,
    status_reason: Option<String>,
    universe_count: i32,
    processed_count: i32,
    result_count: i32,
    error: Option<String>,
    started_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct JobRefRow {
    id: Uuid,
}

#[derive(sqlx::FromRow)]
struct RunnableJobRow {
    id: Uuid,
    user_id: Uuid,
    payload: SqlxJson<Value>,
}

#[derive(sqlx::FromRow)]
struct ScreenerItemRow {
    id: Uuid,
    item_key: String,
    payload: SqlxJson<Value>,
}

enum JobTickOutcome {
    Continue,
    Completed { reason: String },
}

#[derive(sqlx::FromRow)]
struct ResultRow {
    id: Uuid,
    run_id: Uuid,
    run_completed_at: Option<DateTime<Utc>>,
    ticker: String,
    company_name: String,
    ai_tier: Option<String>,
    ai_score: i32,
    status: String,
    current_price: Option<f64>,
    correction_depth: Option<f64>,
    trend_distance: Option<f64>,
    momentum_condition: String,
    volume_condition: String,
    rejection_reason: Option<String>,
    rationale: SqlxJson<Value>,
    rank: i32,
    manual_ai_tier: Option<String>,
    manual_ai_score: Option<i32>,
    manual_status: Option<String>,
    manual_notes: String,
    processed_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct EventRow {
    id: Uuid,
    event_type: String,
    message: String,
    payload: SqlxJson<Value>,
    created_at: DateTime<Utc>,
}

impl From<EventRow> for BackgroundJobEvent {
    fn from(row: EventRow) -> Self {
        Self {
            id: row.id,
            event_type: row.event_type,
            message: row.message,
            payload: row.payload.0,
            created_at: row.created_at,
        }
    }
}

impl From<ResultRow> for AiScreenerResult {
    fn from(row: ResultRow) -> Self {
        Self {
            id: row.id,
            run_id: row.run_id,
            run_completed_at: row.run_completed_at,
            ticker: row.ticker,
            company_name: row.company_name,
            ai_tier: row.ai_tier,
            ai_score: row.ai_score,
            status: row.status,
            current_price: row.current_price,
            correction_depth: row.correction_depth,
            trend_distance: row.trend_distance,
            momentum_condition: row.momentum_condition,
            volume_condition: row.volume_condition,
            rejection_reason: row.rejection_reason,
            rationale: row.rationale.0,
            rank: row.rank,
            manual_ai_tier: row.manual_ai_tier,
            manual_ai_score: row.manual_ai_score,
            manual_status: row.manual_status,
            manual_notes: row.manual_notes,
            processed_at: row.processed_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct DailyPriceCacheStats {
    count: i64,
    earliest_date: Option<NaiveDate>,
    latest_date: Option<NaiveDate>,
    recent_fetched_at: Option<DateTime<Utc>>,
}

#[derive(sqlx::FromRow)]
struct DailyPriceRow {
    date: NaiveDate,
    open: i64,
    high: i64,
    low: i64,
    close: i64,
    volume: Option<i64>,
}

impl From<DailyPriceRow> for PricePoint {
    fn from(row: DailyPriceRow) -> Self {
        Self {
            date: row.date,
            open: cents_f64(row.open),
            high: cents_f64(row.high),
            low: cents_f64(row.low),
            close: cents_f64(row.close),
            volume: row.volume.map(|volume| volume as u64),
        }
    }
}
