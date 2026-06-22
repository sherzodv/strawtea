use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
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

#[derive(Debug, Serialize)]
pub struct CompanyProfile {
    pub ticker: String,
    pub cik: String,
    pub name: String,
    pub entity_type: Option<String>,
    pub sic: Option<String>,
    pub sic_description: Option<String>,
    pub exchanges: Vec<String>,
    pub tickers: Vec<String>,
    pub fiscal_year_end: Option<String>,
    pub state_of_incorporation: Option<String>,
    pub phone: Option<String>,
    pub business_address: Option<CompanyAddress>,
    pub mailing_address: Option<CompanyAddress>,
    pub sec_url: String,
    pub recent_filings: Vec<CompanyFiling>,
    pub financials: Vec<CompanyFinancialMetric>,
}

#[derive(Debug, Serialize)]
pub struct CompanyAddress {
    pub street1: Option<String>,
    pub street2: Option<String>,
    pub city: Option<String>,
    pub state_or_country: Option<String>,
    pub zip_code: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CompanyFiling {
    pub form: String,
    pub filing_date: Option<String>,
    pub report_date: Option<String>,
    pub accession_number: Option<String>,
    pub primary_document: Option<String>,
    pub description: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct CompanyFinancialMetric {
    pub key: String,
    pub label: String,
    pub value: f64,
    pub unit: String,
    pub fiscal_year: Option<i32>,
    pub fiscal_period: Option<String>,
    pub form: Option<String>,
    pub filed: Option<String>,
    pub end: Option<String>,
    pub concept: String,
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

#[derive(Debug, Serialize)]
pub struct AiScreenerRun {
    pub id: Uuid,
    pub job_id: Option<Uuid>,
    pub status: String,
    pub run_after: Option<DateTime<Utc>>,
    pub status_reason: Option<String>,
    pub universe_count: i32,
    pub processed_count: i32,
    pub result_count: i32,
    pub error: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub results: Vec<AiScreenerResult>,
    pub events: Vec<BackgroundJobEvent>,
    pub latest_event: Option<BackgroundJobEvent>,
    pub twelve_budget_used: i32,
    pub twelve_budget_limit: i32,
}

#[derive(Debug, Serialize, Clone)]
pub struct BackgroundJobEvent {
    pub id: Uuid,
    pub event_type: String,
    pub message: String,
    pub payload: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct BackgroundJob {
    pub id: Uuid,
    pub job_type: String,
    pub status: String,
    pub run_after: DateTime<Utc>,
    pub status_reason: Option<String>,
    pub error: Option<String>,
    pub progress_current: i32,
    pub progress_total: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct AiScreenerResult {
    pub id: Uuid,
    pub run_id: Uuid,
    pub run_completed_at: Option<DateTime<Utc>>,
    pub ticker: String,
    pub company_name: String,
    pub ai_tier: Option<String>,
    pub ai_score: i32,
    pub status: String,
    pub current_price: Option<f64>,
    pub correction_depth: Option<f64>,
    pub trend_distance: Option<f64>,
    pub momentum_condition: String,
    pub volume_condition: String,
    pub rejection_reason: Option<String>,
    pub rationale: Value,
    pub rank: i32,
    pub manual_ai_tier: Option<String>,
    pub manual_ai_score: Option<i32>,
    pub manual_status: Option<String>,
    pub manual_notes: String,
    pub processed_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAiScreenerOverride {
    pub manual_ai_tier: Option<String>,
    pub manual_ai_score: Option<i32>,
    pub manual_status: Option<String>,
    pub notes: String,
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

pub type UpdateInvestlogEntry = CreateInvestlogEntry;

#[derive(Debug, Deserialize)]
pub struct AddWatchlistItem {
    pub ticker: String,
    pub note: String,
}

#[derive(Debug, Deserialize)]
pub struct WatchlistRemoval {
    pub note: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateInvestlogTickerNote {
    pub ticker: String,
    pub note: String,
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
pub struct InvestlogAssets {
    pub summary: InvestlogAssetsSummary,
    pub assets: Vec<InvestlogAsset>,
}

#[derive(Debug, Serialize)]
pub struct InvestlogAssetsSummary {
    pub total_buys: i64,
    pub total_sells: i64,
    pub total_commissions: i64,
    pub realized_profit: i64,
    pub unrealized_profit: i64,
    pub net_profit: i64,
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
pub struct InvestlogWatchlistItem {
    pub id: Uuid,
    pub ticker: String,
    pub company_name: Option<String>,
    pub description: Option<String>,
    pub market_cap: Option<i64>,
    pub shares_outstanding: Option<i64>,
    pub revenue: Option<i64>,
    pub total_debt: Option<i64>,
    pub cash: Option<i64>,
    pub free_cash_flow: Option<i64>,
    pub current_price: Option<i64>,
    pub currency: Option<String>,
    pub notes: Vec<InvestlogTickerNote>,
    pub is_active: bool,
    pub removed_at: Option<DateTime<Utc>>,
    pub meta_fetched_at: Option<DateTime<Utc>>,
    pub price_fetched_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct InvestlogTickerNote {
    pub id: Uuid,
    pub ticker: String,
    pub note: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct InvestlogPerformance {
    pub tickers: Vec<String>,
    pub range: String,
    pub series: Vec<InvestlogPerformanceSeries>,
    pub events: Vec<InvestlogPerformanceEvent>,
    pub report_events: Vec<InvestlogReportEvent>,
    pub ticker_notes: Vec<InvestlogTickerNote>,
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
pub struct InvestlogReportEvent {
    pub ticker: String,
    pub date: NaiveDate,
    pub form: String,
    pub filing_date: Option<NaiveDate>,
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
