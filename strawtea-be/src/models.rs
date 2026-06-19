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
