use anyhow::Result;
use sqlx::{PgPool, postgres::PgPoolOptions};

pub async fn connect_db(database_url: &str) -> Result<PgPool> {
    Ok(PgPoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await?)
}
