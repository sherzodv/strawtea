use axum::{
    Json, Router,
    extract::{Multipart, Path, Query, State},
    http::HeaderMap,
    routing::{get, post},
};
use chrono::{DateTime, NaiveDate, Utc};
use tokio::{
    task::spawn_blocking,
    time::{Duration, timeout},
};
use uuid::Uuid;

use crate::{
    error::AppError,
    models::{
        RawtxCategorizationPattern, RawtxImport, RawtxImportPreview, RawtxList, RawtxMonthlySpend,
        RawtxPreviewRow, RawtxRow,
    },
    state::AppState,
    statement_parser::{
        OPTIMA_PARSER_NAME, OPTIMA_PARSER_VERSION, ParsedRawtx, parse_optima_pdf, sha256_hex,
    },
};

pub fn spends_routes() -> Router<AppState> {
    Router::new()
        .route("/spends/monthly", get(monthly_spends))
        .route(
            "/spends/categorization/patterns",
            get(categorization_patterns),
        )
        .route("/spends/rawtx", get(list_rawtx))
        .route("/spends/imports/preview", post(preview_import))
        .route("/spends/imports/{import_id}/confirm", post(confirm_import))
}

#[derive(serde::Deserialize)]
struct RawtxListQuery {
    q: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

async fn monthly_spends(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<RawtxMonthlySpend>>, AppError> {
    let user_id = current_user_id(&state, &headers).await?;

    let rows = sqlx::query_as::<_, RawtxMonthlySpendRow>(
        r#"
        with spend_entries as (
          select
            id,
            date_trunc('month', occurred_at)::date as month,
            operation_currency as currency,
            abs(operation_amount) as amount
          from rawtx
          where user_id = $1
            and operation_amount < 0
            and coalesce(raw_kind, '') not in ('income', 'refund', 'transfer')

          union all

          select
            id,
            date_trunc('month', occurred_at)::date as month,
            fee_currency as currency,
            abs(fee_amount) as amount
          from rawtx
          where user_id = $1
            and fee_amount < 0
        )
        select
          month,
          currency,
          sum(amount)::bigint as amount,
          count(distinct id)::bigint as transaction_count
        from spend_entries
        group by month, currency
        order by month desc, currency
        "#,
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(rows.into_iter().map(Into::into).collect()))
}

async fn categorization_patterns(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<RawtxCategorizationPattern>>, AppError> {
    let user_id = current_user_id(&state, &headers).await?;

    let rows = sqlx::query_as::<_, RawtxCategorizationPatternRow>(
        r#"
        with extracted as (
          select regexp_match(
            r.description_raw,
            '(?i)QR:[[:space:]]*([^[:space:]]+[[:space:]]+[^[:space:]]+)'
          ) as match
          from rawtx r
          where r.user_id = $1
        ),
        matches as (
          select trim(match[1]) as pattern
          from extracted
          where match is not null
            and match[1] is not null
            and trim(match[1]) <> ''
        )
        select
          pattern,
          count(*)::bigint as transaction_count
        from matches
        group by pattern
        order by transaction_count desc, pattern
        "#,
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(rows.into_iter().map(Into::into).collect()))
}

async fn list_rawtx(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<RawtxListQuery>,
) -> Result<Json<RawtxList>, AppError> {
    let user_id = current_user_id(&state, &headers).await?;
    let q = query.q.unwrap_or_default();
    let search = q.trim();
    let search_pattern = if search.is_empty() {
        None
    } else {
        Some(format!("%{search}%"))
    };
    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    let offset = query.offset.unwrap_or(0).max(0);

    let rows = sqlx::query_as::<_, RawtxRowDb>(
        r#"
        select
          r.id,
          i.source_file_name,
          i.bank,
          i.account_number_masked,
          i.card_number_masked,
          i.account_currency,
          r.occurred_at,
          r.posted_date,
          r.description_raw,
          r.operation_amount,
          r.operation_currency,
          r.fee_amount,
          r.fee_currency,
          r.account_amount,
          r.account_amount_currency,
          r.direction,
          r.raw_kind,
          i.parser_name,
          i.parser_version,
          r.created_at
        from rawtx r
        join rawtx_import i on i.id = r.import_id
        where r.user_id = $1
          and (
            $2::text is null
            or r.description_raw ilike $2
            or coalesce(r.raw_kind, '') ilike $2
            or coalesce(i.card_number_masked, '') ilike $2
            or coalesce(i.account_number_masked, '') ilike $2
            or i.source_file_name ilike $2
            or i.bank ilike $2
            or i.account_currency ilike $2
            or r.operation_currency ilike $2
          )
        order by r.occurred_at desc, r.created_at desc
        limit $3 offset $4
        "#,
    )
    .bind(user_id)
    .bind(&search_pattern)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await?;

    let total = sqlx::query_scalar::<_, i64>(
        r#"
        select count(*)::bigint
        from rawtx r
        join rawtx_import i on i.id = r.import_id
        where r.user_id = $1
          and (
            $2::text is null
            or r.description_raw ilike $2
            or coalesce(r.raw_kind, '') ilike $2
            or coalesce(i.card_number_masked, '') ilike $2
            or coalesce(i.account_number_masked, '') ilike $2
            or i.source_file_name ilike $2
            or i.bank ilike $2
            or i.account_currency ilike $2
            or r.operation_currency ilike $2
          )
        "#,
    )
    .bind(user_id)
    .bind(&search_pattern)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(RawtxList {
        rows: rows.into_iter().map(Into::into).collect(),
        total,
        limit,
        offset,
    }))
}

async fn preview_import(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Json<RawtxImportPreview>, AppError> {
    let user_id = current_user_id(&state, &headers).await?;
    let mut file_name = None;
    let mut file_bytes = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|err| AppError::BadRequest(format!("invalid upload: {err}")))?
    {
        if field.name() != Some("file") {
            continue;
        }
        file_name = field.file_name().map(str::to_string);
        file_bytes = Some(
            field
                .bytes()
                .await
                .map_err(|err| AppError::BadRequest(format!("could not read upload: {err}")))?
                .to_vec(),
        );
        break;
    }

    let file_name = file_name.unwrap_or_else(|| "statement.pdf".to_string());
    let file_bytes =
        file_bytes.ok_or_else(|| AppError::BadRequest("file is required".to_string()))?;
    if file_bytes.is_empty() {
        return Err(AppError::BadRequest("file is empty".to_string()));
    }

    let source_file_sha256 = sha256_hex(&file_bytes);
    let parsed = timeout(
        Duration::from_secs(60),
        spawn_blocking(move || parse_optima_pdf(&file_bytes)),
    )
    .await
    .map_err(|_| AppError::BadRequest("PDF parsing timed out".to_string()))?
    .map_err(|err| AppError::BadRequest(format!("PDF parsing task failed: {err}")))??;

    let import_row = sqlx::query_as::<_, RawtxImportRow>(
        r#"
        insert into rawtx_import (
          user_id,
          source_file_name,
          source_file_sha256,
          parser_name,
          parser_version,
          bank,
          account_number_masked,
          card_number_masked,
          account_currency,
          statement_period_start,
          statement_period_end,
          rows_seen
        )
        values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        returning
          id,
          source_file_name,
          source_file_sha256,
          parser_name,
          parser_version,
          bank,
          account_number_masked,
          card_number_masked,
          account_currency,
          statement_period_start,
          statement_period_end,
          status,
          rows_seen,
          rows_inserted,
          rows_duplicate,
          error,
          created_at,
          confirmed_at
        "#,
    )
    .bind(user_id)
    .bind(&file_name)
    .bind(&source_file_sha256)
    .bind(OPTIMA_PARSER_NAME)
    .bind(OPTIMA_PARSER_VERSION)
    .bind(&parsed.bank)
    .bind(&parsed.account_number_masked)
    .bind(&parsed.card_number_masked)
    .bind(&parsed.account_currency)
    .bind(parsed.statement_period_start)
    .bind(parsed.statement_period_end)
    .bind(parsed.rows.len() as i32)
    .fetch_one(&state.db)
    .await?;

    let dedupe_keys: Vec<String> = parsed
        .rows
        .iter()
        .map(|row| row.dedupe_key.clone())
        .collect();
    let existing_keys: Vec<String> = sqlx::query_scalar(
        r#"
        select dedupe_key
        from rawtx
        where user_id = $1
          and dedupe_key = any($2)
        "#,
    )
    .bind(user_id)
    .bind(&dedupe_keys)
    .fetch_all(&state.db)
    .await?;

    let mut duplicate_count = 0;
    for row in parsed.rows {
        let is_duplicate = existing_keys.contains(&row.dedupe_key);
        if is_duplicate {
            duplicate_count += 1;
        }
        insert_preview_row(&state, import_row.id, row, is_duplicate).await?;
    }

    let import_row = sqlx::query_as::<_, RawtxImportRow>(
        r#"
        update rawtx_import
        set rows_duplicate = $2
        where id = $1
        returning
          id,
          source_file_name,
          source_file_sha256,
          parser_name,
          parser_version,
          bank,
          account_number_masked,
          card_number_masked,
          account_currency,
          statement_period_start,
          statement_period_end,
          status,
          rows_seen,
          rows_inserted,
          rows_duplicate,
          error,
          created_at,
          confirmed_at
        "#,
    )
    .bind(import_row.id)
    .bind(duplicate_count)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(read_preview(&state, user_id, import_row.id).await?))
}

async fn confirm_import(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(import_id): Path<Uuid>,
) -> Result<Json<RawtxImportPreview>, AppError> {
    let user_id = current_user_id(&state, &headers).await?;

    let import = sqlx::query_as::<_, RawtxImportRow>(
        r#"
        select
          id,
          source_file_name,
          source_file_sha256,
          parser_name,
          parser_version,
          bank,
          account_number_masked,
          card_number_masked,
          account_currency,
          statement_period_start,
          statement_period_end,
          status,
          rows_seen,
          rows_inserted,
          rows_duplicate,
          error,
          created_at,
          confirmed_at
        from rawtx_import
        where id = $1
          and user_id = $2
        "#,
    )
    .bind(import_id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::BadRequest("import not found".to_string()))?;

    if import.status == "confirmed" {
        return Ok(Json(read_preview(&state, user_id, import_id).await?));
    }

    let result = sqlx::query(
        r#"
        insert into rawtx (
          user_id,
          import_id,
          occurred_at,
          posted_date,
          description_raw,
          row_raw,
          operation_amount,
          operation_currency,
          fee_amount,
          fee_currency,
          account_amount,
          account_amount_currency,
          direction,
          raw_kind,
          semantic_key_base,
          semantic_ordinal,
          dedupe_key,
          raw_fingerprint
        )
        select
          $2,
          i.id,
          r.occurred_at,
          r.posted_date,
          r.description_raw,
          r.row_raw,
          r.operation_amount,
          r.operation_currency,
          r.fee_amount,
          r.fee_currency,
          r.account_amount,
          r.account_amount_currency,
          r.direction,
          r.raw_kind,
          r.semantic_key_base,
          r.semantic_ordinal,
          r.dedupe_key,
          r.raw_fingerprint
        from rawtx_import i
        join rawtx_import_row r on r.import_id = i.id
        where i.id = $1
          and i.user_id = $2
          and r.is_duplicate = false
        on conflict (user_id, dedupe_key) do nothing
        "#,
    )
    .bind(import_id)
    .bind(user_id)
    .execute(&state.db)
    .await?;

    let inserted = result.rows_affected() as i32;
    let duplicate_count = sqlx::query_scalar::<_, i64>(
        r#"
        select count(*)::bigint
        from rawtx_import_row
        where import_id = $1
          and is_duplicate = true
        "#,
    )
    .bind(import_id)
    .fetch_one(&state.db)
    .await? as i32;

    sqlx::query(
        r#"
        update rawtx_import
        set status = 'confirmed',
            rows_inserted = $2,
            rows_duplicate = $3,
            confirmed_at = now()
        where id = $1
          and user_id = $4
        "#,
    )
    .bind(import_id)
    .bind(inserted)
    .bind(duplicate_count)
    .bind(user_id)
    .execute(&state.db)
    .await?;

    Ok(Json(read_preview(&state, user_id, import_id).await?))
}

async fn insert_preview_row(
    state: &AppState,
    import_id: Uuid,
    row: ParsedRawtx,
    is_duplicate: bool,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        insert into rawtx_import_row (
          import_id,
          row_index,
          occurred_at,
          posted_date,
          description_raw,
          row_raw,
          operation_amount,
          operation_currency,
          fee_amount,
          fee_currency,
          account_amount,
          account_amount_currency,
          direction,
          raw_kind,
          semantic_key_base,
          semantic_ordinal,
          dedupe_key,
          raw_fingerprint,
          is_duplicate
        )
        values (
          $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
          $11, $12, $13, $14, $15, $16, $17, $18, $19
        )
        "#,
    )
    .bind(import_id)
    .bind(row.row_index)
    .bind(row.occurred_at)
    .bind(row.posted_date)
    .bind(row.description_raw)
    .bind(row.row_raw)
    .bind(row.operation_amount)
    .bind(row.operation_currency)
    .bind(row.fee_amount)
    .bind(row.fee_currency)
    .bind(row.account_amount)
    .bind(row.account_amount_currency)
    .bind(row.direction)
    .bind(row.raw_kind)
    .bind(row.semantic_key_base)
    .bind(row.semantic_ordinal)
    .bind(row.dedupe_key)
    .bind(row.raw_fingerprint)
    .bind(is_duplicate)
    .execute(&state.db)
    .await?;

    Ok(())
}

async fn read_preview(
    state: &AppState,
    user_id: Uuid,
    import_id: Uuid,
) -> Result<RawtxImportPreview, AppError> {
    let import = sqlx::query_as::<_, RawtxImportRow>(
        r#"
        select
          id,
          source_file_name,
          source_file_sha256,
          parser_name,
          parser_version,
          bank,
          account_number_masked,
          card_number_masked,
          account_currency,
          statement_period_start,
          statement_period_end,
          status,
          rows_seen,
          rows_inserted,
          rows_duplicate,
          error,
          created_at,
          confirmed_at
        from rawtx_import
        where id = $1
          and user_id = $2
        "#,
    )
    .bind(import_id)
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;

    let rows = sqlx::query_as::<_, RawtxPreviewRowDb>(
        r#"
        select
          id,
          row_index,
          occurred_at,
          posted_date,
          description_raw,
          operation_amount,
          operation_currency,
          fee_amount,
          fee_currency,
          account_amount,
          account_amount_currency,
          direction,
          raw_kind,
          is_duplicate
        from rawtx_import_row
        where import_id = $1
        order by row_index
        "#,
    )
    .bind(import_id)
    .fetch_all(&state.db)
    .await?;

    Ok(RawtxImportPreview {
        import: import.into(),
        rows: rows.into_iter().map(Into::into).collect(),
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

#[derive(sqlx::FromRow)]
struct RawtxImportRow {
    id: Uuid,
    source_file_name: String,
    source_file_sha256: String,
    parser_name: String,
    parser_version: i32,
    bank: String,
    account_number_masked: Option<String>,
    card_number_masked: Option<String>,
    account_currency: String,
    statement_period_start: Option<NaiveDate>,
    statement_period_end: Option<NaiveDate>,
    status: String,
    rows_seen: i32,
    rows_inserted: i32,
    rows_duplicate: i32,
    error: Option<String>,
    created_at: DateTime<Utc>,
    confirmed_at: Option<DateTime<Utc>>,
}

#[derive(sqlx::FromRow)]
struct RawtxPreviewRowDb {
    id: Uuid,
    row_index: i32,
    occurred_at: DateTime<Utc>,
    posted_date: Option<NaiveDate>,
    description_raw: String,
    operation_amount: i64,
    operation_currency: String,
    fee_amount: i64,
    fee_currency: String,
    account_amount: Option<i64>,
    account_amount_currency: Option<String>,
    direction: String,
    raw_kind: Option<String>,
    is_duplicate: bool,
}

#[derive(sqlx::FromRow)]
struct RawtxRowDb {
    id: Uuid,
    source_file_name: String,
    bank: String,
    account_number_masked: Option<String>,
    card_number_masked: Option<String>,
    account_currency: String,
    occurred_at: DateTime<Utc>,
    posted_date: Option<NaiveDate>,
    description_raw: String,
    operation_amount: i64,
    operation_currency: String,
    fee_amount: i64,
    fee_currency: String,
    account_amount: Option<i64>,
    account_amount_currency: Option<String>,
    direction: String,
    raw_kind: Option<String>,
    parser_name: String,
    parser_version: i32,
    created_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct RawtxMonthlySpendRow {
    month: NaiveDate,
    currency: String,
    amount: i64,
    transaction_count: i64,
}

#[derive(sqlx::FromRow)]
struct RawtxCategorizationPatternRow {
    pattern: String,
    transaction_count: i64,
}

impl From<RawtxImportRow> for RawtxImport {
    fn from(row: RawtxImportRow) -> Self {
        Self {
            id: row.id,
            source_file_name: row.source_file_name,
            source_file_sha256: row.source_file_sha256,
            parser_name: row.parser_name,
            parser_version: row.parser_version,
            bank: row.bank,
            account_number_masked: row.account_number_masked,
            card_number_masked: row.card_number_masked,
            account_currency: row.account_currency,
            statement_period_start: row.statement_period_start,
            statement_period_end: row.statement_period_end,
            status: row.status,
            rows_seen: row.rows_seen,
            rows_inserted: row.rows_inserted,
            rows_duplicate: row.rows_duplicate,
            error: row.error,
            created_at: row.created_at,
            confirmed_at: row.confirmed_at,
        }
    }
}

impl From<RawtxPreviewRowDb> for RawtxPreviewRow {
    fn from(row: RawtxPreviewRowDb) -> Self {
        Self {
            id: row.id,
            row_index: row.row_index,
            occurred_at: row.occurred_at,
            posted_date: row.posted_date,
            description_raw: row.description_raw,
            operation_amount: row.operation_amount,
            operation_currency: row.operation_currency,
            fee_amount: row.fee_amount,
            fee_currency: row.fee_currency,
            account_amount: row.account_amount,
            account_amount_currency: row.account_amount_currency,
            direction: row.direction,
            raw_kind: row.raw_kind,
            is_duplicate: row.is_duplicate,
        }
    }
}

impl From<RawtxRowDb> for RawtxRow {
    fn from(row: RawtxRowDb) -> Self {
        Self {
            id: row.id,
            source_file_name: row.source_file_name,
            bank: row.bank,
            account_number_masked: row.account_number_masked,
            card_number_masked: row.card_number_masked,
            account_currency: row.account_currency,
            occurred_at: row.occurred_at,
            posted_date: row.posted_date,
            description_raw: row.description_raw,
            operation_amount: row.operation_amount,
            operation_currency: row.operation_currency,
            fee_amount: row.fee_amount,
            fee_currency: row.fee_currency,
            account_amount: row.account_amount,
            account_amount_currency: row.account_amount_currency,
            direction: row.direction,
            raw_kind: row.raw_kind,
            parser_name: row.parser_name,
            parser_version: row.parser_version,
            created_at: row.created_at,
        }
    }
}

impl From<RawtxMonthlySpendRow> for RawtxMonthlySpend {
    fn from(row: RawtxMonthlySpendRow) -> Self {
        Self {
            month: row.month,
            currency: row.currency,
            amount: row.amount,
            transaction_count: row.transaction_count,
        }
    }
}

impl From<RawtxCategorizationPatternRow> for RawtxCategorizationPattern {
    fn from(row: RawtxCategorizationPatternRow) -> Self {
        Self {
            pattern: row.pattern,
            transaction_count: row.transaction_count,
        }
    }
}
