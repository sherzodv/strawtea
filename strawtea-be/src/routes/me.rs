use axum::{Json, Router, extract::State, http::HeaderMap, routing::get};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::{error::AppError, models::CurrentUser, state::AppState};

pub fn me_routes() -> Router<AppState> {
    Router::new().route("/me", get(me))
}

async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<CurrentUser>, AppError> {
    let auth_user = state.auth.user_from_headers(&headers)?;

    let row: (Uuid, String, DateTime<Utc>) = sqlx::query_as(
        r#"
        insert into users (supabase_user_id, email)
        values ($1, $2)
        on conflict (supabase_user_id) do update
        set email = excluded.email,
            updated_at = now()
        returning id, email, created_at
        "#,
    )
    .bind(auth_user.supabase_user_id)
    .bind(auth_user.email)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(CurrentUser {
        id: row.0,
        email: row.1,
        created_at: row.2,
    }))
}
