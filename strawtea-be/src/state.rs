use sqlx::PgPool;

use crate::{
    auth::SupabaseAuth,
    integrations::{
        edgar::EdgarClient, market_data::TwelveDataClient, openai::OpenAiClient,
        push::PushNotifications,
    },
};

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub auth: SupabaseAuth,
    pub market_data: TwelveDataClient,
    pub edgar: EdgarClient,
    pub openai: Option<OpenAiClient>,
    pub push: Option<PushNotifications>,
}
