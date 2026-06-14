use axum::http::HeaderMap;
use jsonwebtoken::{
    DecodingKey, Validation, decode, decode_header,
    jwk::{AlgorithmParameters, JwkSet},
};
use serde::Deserialize;

use crate::error::AppError;

#[derive(Clone)]
pub struct SupabaseAuth {
    issuer: String,
    audience: String,
    jwks: JwkSet,
}

#[derive(Debug, Deserialize)]
struct SupabaseClaims {
    sub: String,
    email: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub supabase_user_id: String,
    pub email: String,
}

impl SupabaseAuth {
    pub async fn new(issuer: String, audience: String, jwks_url: String) -> Result<Self, AppError> {
        let jwks = reqwest::get(&jwks_url).await?.json::<JwkSet>().await?;

        Ok(Self {
            issuer,
            audience,
            jwks,
        })
    }

    pub fn user_from_headers(&self, headers: &HeaderMap) -> Result<AuthUser, AppError> {
        let token = bearer_token(headers)?;
        self.verify(token)
    }

    fn verify(&self, token: &str) -> Result<AuthUser, AppError> {
        let header = decode_header(token)?;
        let kid = header.kid.ok_or(AppError::Unauthorized)?;
        let jwk = self.jwks.find(&kid).ok_or(AppError::Unauthorized)?;

        match &jwk.algorithm {
            AlgorithmParameters::RSA(_) | AlgorithmParameters::EllipticCurve(_) => {}
            _ => return Err(AppError::Unauthorized),
        }

        let algorithm = header.alg;
        let decoding_key = DecodingKey::from_jwk(jwk)?;

        let mut validation = Validation::new(algorithm);
        validation.set_audience(&[self.audience.as_str()]);
        validation.set_issuer(&[self.issuer.as_str()]);

        let claims = decode::<SupabaseClaims>(token, &decoding_key, &validation)?.claims;
        let email = claims.email.ok_or(AppError::Unauthorized)?;

        Ok(AuthUser {
            supabase_user_id: claims.sub,
            email,
        })
    }
}

fn bearer_token(headers: &HeaderMap) -> Result<&str, AppError> {
    let value = headers
        .get(axum::http::header::AUTHORIZATION)
        .ok_or(AppError::Unauthorized)?
        .to_str()
        .map_err(|_| AppError::Unauthorized)?;

    value
        .strip_prefix("Bearer ")
        .filter(|token| !token.is_empty())
        .ok_or(AppError::Unauthorized)
}
