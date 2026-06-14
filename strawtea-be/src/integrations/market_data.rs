use chrono::NaiveDate;
use reqwest::Url;
use serde::Deserialize;

use crate::{
    error::AppError,
    models::{PricePoint, TickerSearchResult},
};

#[derive(Clone)]
pub struct TwelveDataClient {
    api_key: String,
    client: reqwest::Client,
}

impl TwelveDataClient {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: reqwest::Client::new(),
        }
    }

    pub async fn search(&self, query: &str) -> Result<Vec<TickerSearchResult>, AppError> {
        let mut url = Url::parse("https://api.twelvedata.com/symbol_search")
            .map_err(|err| AppError::MarketData(err.to_string()))?;
        url.query_pairs_mut()
            .append_pair("symbol", query)
            .append_pair("apikey", &self.api_key);

        let response = self.client.get(url).send().await?.error_for_status()?;
        let payload = response.json::<SymbolSearchResponse>().await?;

        Ok(payload
            .data
            .unwrap_or_default()
            .into_iter()
            .filter_map(|item| {
                let symbol = item.symbol?;
                Some(TickerSearchResult {
                    name: item.instrument_name.unwrap_or_else(|| symbol.clone()),
                    symbol,
                    exchange: item.exchange,
                    asset_type: item.instrument_type,
                })
            })
            .collect())
    }

    pub async fn price_history(&self, ticker: &str) -> Result<Vec<PricePoint>, AppError> {
        let mut url = Url::parse("https://api.twelvedata.com/time_series")
            .map_err(|err| AppError::MarketData(err.to_string()))?;
        url.query_pairs_mut()
            .append_pair("symbol", ticker)
            .append_pair("interval", "1day")
            .append_pair("outputsize", "31")
            .append_pair("order", "ASC")
            .append_pair("apikey", &self.api_key);

        let response = self.client.get(url).send().await?.error_for_status()?;
        let payload = response.json::<TimeSeriesResponse>().await?;

        if let Some(message) = payload.message.or(payload.note) {
            return Err(AppError::MarketData(message));
        }

        payload
            .values
            .unwrap_or_default()
            .into_iter()
            .map(PriceValue::try_into)
            .collect()
    }

    pub async fn latest_price_cents(&self, ticker: &str) -> Result<i64, AppError> {
        let mut url = Url::parse("https://api.twelvedata.com/price")
            .map_err(|err| AppError::MarketData(err.to_string()))?;
        url.query_pairs_mut()
            .append_pair("symbol", ticker)
            .append_pair("apikey", &self.api_key);

        let response = self.client.get(url).send().await?.error_for_status()?;
        let payload = response.json::<LatestPriceResponse>().await?;

        if let Some(message) = payload.message.or(payload.note) {
            return Err(AppError::MarketData(message));
        }

        let price = payload
            .price
            .ok_or_else(|| AppError::MarketData("latest price missing".to_string()))?;

        parse_cents(&price)
    }
}

#[derive(Deserialize)]
struct LatestPriceResponse {
    price: Option<String>,
    message: Option<String>,
    note: Option<String>,
}

#[derive(Deserialize)]
struct SymbolSearchResponse {
    data: Option<Vec<SymbolSearchItem>>,
}

#[derive(Deserialize)]
struct SymbolSearchItem {
    symbol: Option<String>,
    #[serde(rename = "instrument_name")]
    instrument_name: Option<String>,
    exchange: Option<String>,
    #[serde(rename = "instrument_type")]
    instrument_type: Option<String>,
}

#[derive(Deserialize)]
struct TimeSeriesResponse {
    values: Option<Vec<PriceValue>>,
    message: Option<String>,
    note: Option<String>,
}

#[derive(Deserialize)]
struct PriceValue {
    datetime: String,
    open: String,
    high: String,
    low: String,
    close: String,
    volume: Option<String>,
}

impl TryFrom<PriceValue> for PricePoint {
    type Error = AppError;

    fn try_from(value: PriceValue) -> Result<Self, Self::Error> {
        Ok(Self {
            date: NaiveDate::parse_from_str(&value.datetime, "%Y-%m-%d")
                .map_err(|err| AppError::MarketData(err.to_string()))?,
            open: parse_f64(&value.open)?,
            high: parse_f64(&value.high)?,
            low: parse_f64(&value.low)?,
            close: parse_f64(&value.close)?,
            volume: value.volume.as_deref().map(parse_u64).transpose()?,
        })
    }
}

fn parse_f64(value: &str) -> Result<f64, AppError> {
    value
        .parse::<f64>()
        .map_err(|err| AppError::MarketData(err.to_string()))
}

fn parse_u64(value: &str) -> Result<u64, AppError> {
    value
        .parse::<u64>()
        .map_err(|err| AppError::MarketData(err.to_string()))
}

fn parse_cents(value: &str) -> Result<i64, AppError> {
    let parsed = parse_f64(value)?;
    Ok((parsed * 100.0).round() as i64)
}
