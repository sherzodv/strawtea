use chrono::NaiveDate;
use reqwest::Url;
use serde::Deserialize;

use crate::{
    error::AppError,
    integrations::throttle::{JobBudget, ProviderThrottle},
    models::{PricePoint, TickerSearchResult},
};

#[derive(Clone)]
pub struct TwelveDataClient {
    api_key: String,
    client: reqwest::Client,
    throttle: ProviderThrottle,
}

impl TwelveDataClient {
    pub fn new(api_key: String, throttle: ProviderThrottle) -> Self {
        Self {
            api_key,
            client: reqwest::Client::new(),
            throttle,
        }
    }

    pub async fn search(&self, query: &str) -> Result<Vec<TickerSearchResult>, AppError> {
        self.throttle
            .reserve("twelvedata", "symbol_search", 1)
            .await?;

        let mut url = Url::parse("https://api.twelvedata.com/symbol_search")
            .map_err(|err| AppError::MarketData(err.to_string()))?;
        url.query_pairs_mut()
            .append_pair("symbol", query)
            .append_pair("apikey", &self.api_key);

        let response = self.get(url, "symbol_search").await?;
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
        self.daily_price_history(ticker, 31).await
    }

    pub async fn daily_price_history(
        &self,
        ticker: &str,
        outputsize: u16,
    ) -> Result<Vec<PricePoint>, AppError> {
        self.daily_price_history_with_budget(ticker, outputsize, None)
            .await
    }

    pub async fn daily_price_history_with_budget(
        &self,
        ticker: &str,
        outputsize: u16,
        job_budget: Option<JobBudget>,
    ) -> Result<Vec<PricePoint>, AppError> {
        self.throttle
            .reserve_with_job_budget("twelvedata", "time_series", 1, job_budget)
            .await?;

        let mut url = Url::parse("https://api.twelvedata.com/time_series")
            .map_err(|err| AppError::MarketData(err.to_string()))?;
        url.query_pairs_mut()
            .append_pair("symbol", ticker)
            .append_pair("interval", "1day")
            .append_pair("outputsize", &outputsize.to_string())
            .append_pair("order", "ASC")
            .append_pair("apikey", &self.api_key);

        let response = self.get(url, "time_series").await?;
        let payload = response.json::<TimeSeriesResponse>().await?;

        if let Some(message) = payload.message.or(payload.note) {
            return Err(AppError::MarketData(message));
        }

        payload
            .values
            .unwrap_or_default()
            .into_iter()
            .filter_map(|value| {
                let date = value.datetime.clone();
                match PricePoint::try_from(value) {
                    Ok(point) => Some(Ok(point)),
                    Err(AppError::MarketData(message))
                        if message.starts_with("invalid non-positive price bar") =>
                    {
                        tracing::debug!(ticker, date, reason = %message, "skipping invalid price bar");
                        None
                    }
                    Err(err) => Some(Err(err)),
                }
            })
            .collect()
    }

    pub async fn latest_price_cents(&self, ticker: &str) -> Result<i64, AppError> {
        self.throttle.reserve("twelvedata", "price", 1).await?;

        let mut url = Url::parse("https://api.twelvedata.com/price")
            .map_err(|err| AppError::MarketData(err.to_string()))?;
        url.query_pairs_mut()
            .append_pair("symbol", ticker)
            .append_pair("apikey", &self.api_key);

        let response = self.get(url, "price").await?;
        let payload = response.json::<LatestPriceResponse>().await?;

        if let Some(message) = payload.message.or(payload.note) {
            return Err(AppError::MarketData(message));
        }

        let price = payload
            .price
            .ok_or_else(|| AppError::MarketData("latest price missing".to_string()))?;

        parse_cents(&price)
    }

    async fn get(&self, url: Url, endpoint: &str) -> Result<reqwest::Response, AppError> {
        let response = self.client.get(url).send().await?;
        let status = response.status();
        if !status.is_success() {
            if status == reqwest::StatusCode::NOT_FOUND {
                return Err(AppError::MarketDataNotFound(format!(
                    "Twelve Data {endpoint} data was not found"
                )));
            }
            return Err(AppError::MarketData(format!(
                "Twelve Data {endpoint} request failed with status {status}"
            )));
        }

        Ok(response)
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
        let date = NaiveDate::parse_from_str(&value.datetime, "%Y-%m-%d")
            .map_err(|err| AppError::MarketData(err.to_string()))?;
        let open = parse_f64(&value.open)?;
        let high = parse_f64(&value.high)?;
        let low = parse_f64(&value.low)?;
        let close = parse_f64(&value.close)?;
        validate_price_bar(date, open, high, low, close)?;

        Ok(Self {
            date,
            open,
            high,
            low,
            close,
            volume: value.volume.as_deref().map(parse_u64).transpose()?,
        })
    }
}

fn validate_price_bar(
    date: NaiveDate,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
) -> Result<(), AppError> {
    let values = [open, high, low, close];
    if values
        .iter()
        .any(|value| !value.is_finite() || f64_cents(*value) <= 0)
    {
        return Err(AppError::MarketData(format!(
            "invalid non-positive price bar for {date}"
        )));
    }

    Ok(())
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

fn f64_cents(value: f64) -> i64 {
    (value * 100.0).round() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_positive_price_bars() {
        let value = PriceValue {
            datetime: "2026-06-23".to_string(),
            open: "1.00".to_string(),
            high: "1.20".to_string(),
            low: "0".to_string(),
            close: "1.10".to_string(),
            volume: None,
        };

        assert!(PricePoint::try_from(value).is_err());
    }

    #[test]
    fn rejects_prices_that_round_to_zero_cents() {
        let value = PriceValue {
            datetime: "2026-06-23".to_string(),
            open: "0.004".to_string(),
            high: "0.004".to_string(),
            low: "0.004".to_string(),
            close: "0.004".to_string(),
            volume: None,
        };

        assert!(PricePoint::try_from(value).is_err());
    }
}

fn parse_cents(value: &str) -> Result<i64, AppError> {
    let parsed = parse_f64(value)?;
    Ok((parsed * 100.0).round() as i64)
}
