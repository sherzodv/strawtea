use std::env;

use anyhow::{Context, Result};

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub supabase_jwt_issuer: String,
    pub supabase_jwt_audience: String,
    pub supabase_jwt_jwks_url: String,
    pub twelve_data_api_key: String,
    pub http_addr: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            database_url: required_env("DATABASE_URL")?,
            supabase_jwt_issuer: required_env("SUPABASE_JWT_ISSUER")?,
            supabase_jwt_audience: required_env("SUPABASE_JWT_AUDIENCE")?,
            supabase_jwt_jwks_url: required_env("SUPABASE_JWT_JWKS_URL")?,
            twelve_data_api_key: required_env("TWELVE_DATA_API_KEY")?,
            http_addr: env::var("HTTP_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".to_string()),
        })
    }
}

fn required_env(name: &str) -> Result<String> {
    env::var(name).with_context(|| format!("{name} must be set"))
}
