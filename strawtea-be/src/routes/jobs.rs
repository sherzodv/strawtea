use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use serde_json::json;
use sqlx::types::Json as SqlxJson;
use uuid::Uuid;

use crate::{error::AppError, models::BackgroundJob, state::AppState};

pub fn job_routes() -> Router<AppState> {
    Router::new()
        .route("/jobs", get(list_jobs))
        .route("/jobs/{id}", get(read_job))
        .route("/jobs/{id}/stop", post(stop_job))
        .route("/jobs/{id}/resume", post(resume_job))
        .route("/jobs/{id}/abort", post(abort_job))
}

async fn list_jobs(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<BackgroundJob>>, AppError> {
    let user_id = current_user_id(&state, &headers).await?;
    let rows = sqlx::query_as::<_, JobRow>(
        r#"
        select
          id,
          job_type,
          status,
          run_after,
          status_reason,
          error,
          progress_current,
          progress_total,
          created_at,
          updated_at
        from background_jobs
        where user_id = $1
        order by created_at desc
        limit 50
        "#,
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(rows.into_iter().map(Into::into).collect()))
}

async fn read_job(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<BackgroundJob>, AppError> {
    let user_id = current_user_id(&state, &headers).await?;
    read_owned_job(&state, user_id, id).await.map(Json)
}

async fn stop_job(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<BackgroundJob>, AppError> {
    let user_id = current_user_id(&state, &headers).await?;
    update_job_status(
        &state,
        user_id,
        id,
        "stopped",
        "stopped by user",
        "stopped_at = now(),",
    )
    .await?;
    log_job_event(&state, id, "job_stopped", "Job stopped by user").await?;
    mirror_screener_job_status(&state, id).await?;
    read_owned_job(&state, user_id, id).await.map(Json)
}

async fn resume_job(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<BackgroundJob>, AppError> {
    let user_id = current_user_id(&state, &headers).await?;
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
          and user_id = $2
          and status in ('stopped', 'waiting_rate_limit', 'failed')
        "#,
    )
    .bind(id)
    .bind(user_id)
    .execute(&state.db)
    .await?;
    log_job_event(&state, id, "job_resumed", "Job resumed by user").await?;
    mirror_screener_job_status(&state, id).await?;
    read_owned_job(&state, user_id, id).await.map(Json)
}

async fn abort_job(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<BackgroundJob>, AppError> {
    let user_id = current_user_id(&state, &headers).await?;
    update_job_status(
        &state,
        user_id,
        id,
        "aborted",
        "aborted by user",
        "aborted_at = now(),",
    )
    .await?;
    log_job_event(&state, id, "job_aborted", "Job aborted by user").await?;
    mirror_screener_job_status(&state, id).await?;
    read_owned_job(&state, user_id, id).await.map(Json)
}

async fn log_job_event(
    state: &AppState,
    job_id: Uuid,
    event_type: &str,
    message: &str,
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
    .bind(SqlxJson(json!({})))
    .execute(&state.db)
    .await?;

    Ok(())
}

async fn update_job_status(
    state: &AppState,
    user_id: Uuid,
    id: Uuid,
    status: &str,
    reason: &str,
    timestamp_assignment: &str,
) -> Result<(), AppError> {
    let sql = format!(
        r#"
        update background_jobs
        set status = $3,
            status_reason = $4,
            {timestamp_assignment}
            updated_at = now()
        where id = $1
          and user_id = $2
          and status not in ('completed', 'aborted')
        "#
    );

    sqlx::query(&sql)
        .bind(id)
        .bind(user_id)
        .bind(status)
        .bind(reason)
        .execute(&state.db)
        .await?;

    Ok(())
}

async fn read_owned_job(
    state: &AppState,
    user_id: Uuid,
    id: Uuid,
) -> Result<BackgroundJob, AppError> {
    let row = sqlx::query_as::<_, JobRow>(
        r#"
        select
          id,
          job_type,
          status,
          run_after,
          status_reason,
          error,
          progress_current,
          progress_total,
          created_at,
          updated_at
        from background_jobs
        where id = $1
          and user_id = $2
        "#,
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("job not found".to_string()))?;

    Ok(row.into())
}

async fn mirror_screener_job_status(state: &AppState, job_id: Uuid) -> Result<(), AppError> {
    sqlx::query(
        r#"
        update ai_screener_runs r
        set status = j.status,
            run_after = j.run_after,
            status_reason = j.status_reason,
            error = j.error,
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

#[derive(sqlx::FromRow)]
struct JobRow {
    id: Uuid,
    job_type: String,
    status: String,
    run_after: DateTime<Utc>,
    status_reason: Option<String>,
    error: Option<String>,
    progress_current: i32,
    progress_total: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<JobRow> for BackgroundJob {
    fn from(row: JobRow) -> Self {
        Self {
            id: row.id,
            job_type: row.job_type,
            status: row.status,
            run_after: row.run_after,
            status_reason: row.status_reason,
            error: row.error,
            progress_current: row.progress_current,
            progress_total: row.progress_total,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}
