use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("unauthorized")]
    Unauthorized,
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("market data error: {0}")]
    MarketData(String),
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
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::MarketData(_) => StatusCode::BAD_GATEWAY,
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
