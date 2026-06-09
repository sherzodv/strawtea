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
