use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct CurrentUser {
    pub id: Uuid,
    pub email: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct TickerSearchResult {
    pub symbol: String,
    pub name: String,
    pub exchange: Option<String>,
    pub asset_type: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PriceHistory {
    pub ticker: String,
    pub range: String,
    pub prices: Vec<PricePoint>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PricePoint {
    pub date: NaiveDate,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct CreateInvestlogEntry {
    pub ticker: String,
    pub occurred_at: DateTime<Utc>,
    pub op: String,
    pub broker: String,
    pub currency: String,
    pub price: i64,
    pub quantity: i64,
    pub fees: i64,
    pub notes: String,
}

#[derive(Debug, Serialize)]
pub struct InvestlogEntry {
    pub id: Uuid,
    pub ticker: String,
    pub occurred_at: DateTime<Utc>,
    pub op: String,
    pub broker: String,
    pub currency: String,
    pub price: i64,
    pub quantity: i64,
    pub fees: i64,
    pub notes: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct InvestlogAsset {
    pub ticker: String,
    pub days_since_buy_midpoint: i64,
    pub quantity: i64,
    pub avg_buy_price: i64,
    pub cost: i64,
    pub current_price: i64,
    pub price_change: i64,
    pub current_value: i64,
    pub amount_change: i64,
    pub percent_change: f64,
    pub price_fetched_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct InvestlogPerformance {
    pub tickers: Vec<String>,
    pub range: String,
    pub series: Vec<InvestlogPerformanceSeries>,
    pub events: Vec<InvestlogPerformanceEvent>,
}

#[derive(Debug, Serialize)]
pub struct InvestlogPerformanceSeries {
    pub ticker: String,
    pub points: Vec<InvestlogPerformancePoint>,
}

#[derive(Debug, Serialize)]
pub struct InvestlogPerformancePoint {
    pub date: NaiveDate,
    pub close: i64,
    pub index: f64,
}

#[derive(Debug, Serialize)]
pub struct InvestlogPerformanceEvent {
    pub ticker: String,
    pub date: NaiveDate,
    pub op: String,
    pub price: i64,
    pub quantity: i64,
    pub notes: String,
}

#[derive(Debug, Serialize)]
pub struct RawtxImport {
    pub id: Uuid,
    pub source_file_name: String,
    pub source_file_sha256: String,
    pub parser_name: String,
    pub parser_version: i32,
    pub bank: String,
    pub account_number_masked: Option<String>,
    pub card_number_masked: Option<String>,
    pub account_currency: String,
    pub statement_period_start: Option<NaiveDate>,
    pub statement_period_end: Option<NaiveDate>,
    pub status: String,
    pub rows_seen: i32,
    pub rows_inserted: i32,
    pub rows_duplicate: i32,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub confirmed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct RawtxPreviewRow {
    pub id: Uuid,
    pub row_index: i32,
    pub occurred_at: DateTime<Utc>,
    pub posted_date: Option<NaiveDate>,
    pub description_raw: String,
    pub operation_amount: i64,
    pub operation_currency: String,
    pub fee_amount: i64,
    pub fee_currency: String,
    pub account_amount: Option<i64>,
    pub account_amount_currency: Option<String>,
    pub direction: String,
    pub raw_kind: Option<String>,
    pub is_duplicate: bool,
}

#[derive(Debug, Serialize)]
pub struct RawtxImportPreview {
    pub import: RawtxImport,
    pub rows: Vec<RawtxPreviewRow>,
}

#[derive(Debug, Serialize)]
pub struct RawtxList {
    pub rows: Vec<RawtxRow>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Serialize)]
pub struct RawtxMonthlySpend {
    pub month: NaiveDate,
    pub currency: String,
    pub amount: i64,
    pub transaction_count: i64,
}

#[derive(Debug, Serialize)]
pub struct RawtxCategorizationPattern {
    pub pattern: String,
    pub transaction_count: i64,
}

#[derive(Debug, Serialize)]
pub struct RawtxRow {
    pub id: Uuid,
    pub source_file_name: String,
    pub bank: String,
    pub account_number_masked: Option<String>,
    pub card_number_masked: Option<String>,
    pub account_currency: String,
    pub occurred_at: DateTime<Utc>,
    pub posted_date: Option<NaiveDate>,
    pub description_raw: String,
    pub operation_amount: i64,
    pub operation_currency: String,
    pub fee_amount: i64,
    pub fee_currency: String,
    pub account_amount: Option<i64>,
    pub account_amount_currency: Option<String>,
    pub direction: String,
    pub raw_kind: Option<String>,
    pub parser_name: String,
    pub parser_version: i32,
    pub created_at: DateTime<Utc>,
}
