use std::env;

use anyhow::{Context, Result};

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub supabase_jwt_issuer: String,
    pub supabase_jwt_audience: String,
    pub supabase_jwt_jwks_url: String,
    pub twelve_data_api_key: String,
    pub sec_user_agent: String,
    pub openai_api_key: Option<String>,
    pub openai_model: String,
    pub http_addr: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let _ = dotenvy::dotenv();

        Ok(Self {
            database_url: required_env("DATABASE_URL")?,
            supabase_jwt_issuer: required_env("SUPABASE_JWT_ISSUER")?,
            supabase_jwt_audience: required_env("SUPABASE_JWT_AUDIENCE")?,
            supabase_jwt_jwks_url: required_env("SUPABASE_JWT_JWKS_URL")?,
            twelve_data_api_key: env::var("STRAWTEA_TWELVE_API_KEY")
                .or_else(|_| env::var("TWELVE_DATA_API_KEY"))
                .context("STRAWTEA_TWELVE_API_KEY or TWELVE_DATA_API_KEY must be set")?,
            sec_user_agent: required_env("STRAWTEA_SEC_USER_AGENT")?,
            openai_api_key: env::var("STRAWTEA_OPENAI_API_KEY").ok(),
            openai_model: env::var("STRAWTEA_OPENAI_MODEL")
                .unwrap_or_else(|_| "gpt-5.2".to_string()),
            http_addr: env::var("HTTP_ADDR")
                .or_else(|_| env::var("PORT").map(|port| format!("0.0.0.0:{port}")))
                .unwrap_or_else(|_| "127.0.0.1:8080".to_string()),
        })
    }
}

fn required_env(name: &str) -> Result<String> {
    env::var(name).with_context(|| format!("{name} must be set"))
}
