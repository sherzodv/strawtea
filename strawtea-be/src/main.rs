mod auth;
mod config;
mod db;
mod error;
mod integrations;
mod models;
mod routes;
mod state;

use std::net::SocketAddr;

use anyhow::Context;
use axum::Router;
use tokio::net::TcpListener;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::{
    auth::SupabaseAuth,
    config::Config,
    db::connect_db,
    integrations::market_data::TwelveDataClient,
    routes::{
        health::health_routes, investlog::investlog_routes, me::me_routes, stocks::stock_routes,
    },
    state::AppState,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "strawtea_be=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env()?;
    let db = connect_db(&config.database_url).await?;
    let auth = SupabaseAuth::new(
        config.supabase_jwt_issuer.clone(),
        config.supabase_jwt_audience.clone(),
        config.supabase_jwt_jwks_url.clone(),
    )
    .await?;
    let market_data = TwelveDataClient::new(config.twelve_data_api_key.clone());

    let state = AppState {
        db,
        auth,
        market_data,
    };

    let app = Router::new()
        .merge(health_routes())
        .nest(
            "/api",
            me_routes().merge(stock_routes()).merge(investlog_routes()),
        )
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr: SocketAddr = config.http_addr.parse().context("invalid HTTP_ADDR")?;
    let listener = TcpListener::bind(addr).await?;
    tracing::info!(%addr, "strawtea backend listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
