use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("unauthorized")]
    Unauthorized,
    #[error("not found: {0}")]
    NotFound(String),
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("SEC EDGAR error: {0}")]
    Edgar(String),
    #[error("market data error: {0}")]
    MarketData(String),
    #[error("market data not found: {0}")]
    MarketDataNotFound(String),
    #[error("AI enrichment error: {0}")]
    Ai(String),
    #[error("rate limited by {provider}; retry after {retry_after}")]
    RateLimited {
        provider: String,
        retry_after: DateTime<Utc>,
    },
    #[error("{provider} job budget exhausted: {used}/{limit} credits")]
    JobBudgetExhausted {
        provider: String,
        used: i32,
        limit: i32,
    },
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("http client error: {0}")]
    HttpClient(#[from] reqwest::Error),
    #[error("jwt error: {0}")]
    Jwt(#[from] jsonwebtoken::errors::Error),
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match self {
            AppError::Unauthorized => StatusCode::UNAUTHORIZED,
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::RateLimited { .. } => StatusCode::TOO_MANY_REQUESTS,
            AppError::JobBudgetExhausted { .. } => StatusCode::TOO_MANY_REQUESTS,
            AppError::MarketDataNotFound(_) => StatusCode::NOT_FOUND,
            AppError::Edgar(_) | AppError::MarketData(_) | AppError::Ai(_) => {
                StatusCode::BAD_GATEWAY
            }
            AppError::Database(_) | AppError::HttpClient(_) | AppError::Jwt(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };

        let body = Json(ErrorBody {
            error: self.to_string(),
        });

        (status, body).into_response()
    }
}
