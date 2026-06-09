use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::get,
};
use serde::Deserialize;

use crate::{error::AppError, models::PriceHistory, state::AppState};

pub fn stock_routes() -> Router<AppState> {
    Router::new()
        .route("/stocks/search", get(search))
        .route("/stocks/{ticker}/prices", get(prices))
}

#[derive(Deserialize)]
struct SearchQuery {
    q: String,
}

#[derive(Deserialize)]
struct PriceQuery {
    range: Option<String>,
}

async fn search(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<SearchQuery>,
) -> Result<Json<Vec<crate::models::TickerSearchResult>>, AppError> {
    state.auth.user_from_headers(&headers)?;

    if query.q.trim().len() < 2 {
        return Ok(Json(Vec::new()));
    }

    Ok(Json(state.market_data.search(query.q.trim()).await?))
}

async fn prices(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(ticker): Path<String>,
    Query(query): Query<PriceQuery>,
) -> Result<Json<PriceHistory>, AppError> {
    state.auth.user_from_headers(&headers)?;

    let range = query.range.unwrap_or_else(|| "1mo".to_string());
    if range != "1mo" {
        return Err(AppError::BadRequest(
            "only range=1mo is supported".to_string(),
        ));
    }

    let ticker = ticker.trim().to_uppercase();
    if ticker.is_empty() {
        return Err(AppError::BadRequest("ticker is required".to_string()));
    }

    let prices = state.market_data.price_history(&ticker).await?;

    Ok(Json(PriceHistory {
        ticker,
        range,
        prices,
    }))
}
