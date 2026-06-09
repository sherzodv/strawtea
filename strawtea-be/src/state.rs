use sqlx::PgPool;

use crate::{auth::SupabaseAuth, integrations::market_data::TwelveDataClient};

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub auth: SupabaseAuth,
    pub market_data: TwelveDataClient,
}
