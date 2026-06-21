use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::get,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::types::Json as SqlJson;
use uuid::Uuid;

use crate::{error::AppError, state::AppState};

pub fn settings_routes() -> Router<AppState> {
    Router::new().route(
        "/settings/{section}/{setting_key}",
        get(get_setting).put(put_setting),
    )
}

#[derive(Deserialize)]
struct SettingPath {
    section: String,
    setting_key: String,
}

#[derive(Deserialize)]
struct PutSettingRequest {
    value: Value,
}

#[derive(Serialize)]
struct SettingResponse {
    value: Option<Value>,
}

async fn get_setting(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<SettingPath>,
) -> Result<Json<SettingResponse>, AppError> {
    validate_namespace(&path.section, &path.setting_key)?;
    let user_id = current_user_id(&state, &headers).await?;

    let row: Option<(SqlJson<Value>,)> = sqlx::query_as(
        r#"
        select value
        from user_settings
        where user_id = $1
          and section = $2
          and setting_key = $3
        "#,
    )
    .bind(user_id)
    .bind(path.section)
    .bind(path.setting_key)
    .fetch_optional(&state.db)
    .await?;

    Ok(Json(SettingResponse {
        value: row.map(|item| item.0.0),
    }))
}

async fn put_setting(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<SettingPath>,
    Json(payload): Json<PutSettingRequest>,
) -> Result<Json<SettingResponse>, AppError> {
    validate_namespace(&path.section, &path.setting_key)?;
    let user_id = current_user_id(&state, &headers).await?;

    let row: (SqlJson<Value>,) = sqlx::query_as(
        r#"
        insert into user_settings (
          user_id,
          section,
          setting_key,
          value
        )
        values ($1, $2, $3, $4)
        on conflict (user_id, section, setting_key) do update
        set value = excluded.value,
            updated_at = now()
        returning value
        "#,
    )
    .bind(user_id)
    .bind(path.section)
    .bind(path.setting_key)
    .bind(SqlJson(payload.value))
    .fetch_one(&state.db)
    .await?;

    Ok(Json(SettingResponse {
        value: Some(row.0.0),
    }))
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

fn validate_namespace(section: &str, setting_key: &str) -> Result<(), AppError> {
    validate_namespace_part(section, "section")?;
    validate_namespace_part(setting_key, "setting key")?;
    Ok(())
}

fn validate_namespace_part(value: &str, label: &str) -> Result<(), AppError> {
    if value.is_empty() || value.len() > 64 {
        return Err(AppError::BadRequest(format!("{label} is invalid")));
    }

    if !value
        .chars()
        .all(|item| item.is_ascii_lowercase() || item.is_ascii_digit() || matches!(item, '-' | '_'))
    {
        return Err(AppError::BadRequest(format!("{label} is invalid")));
    }

    Ok(())
}
