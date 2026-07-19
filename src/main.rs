#![warn(clippy::all, clippy::pedantic)]
#![deny(clippy::arithmetic_side_effects)]
#![cfg_attr(not(test), deny(clippy::unwrap_used))]
#![allow(clippy::duration_suboptimal_units)]

use std::time::Duration;

use dotenvy::dotenv;
use poem::{EndpointExt, Route, Server, listener::TcpListener, middleware::AddData};
use tokio::signal;

mod auth;
mod background;
mod config;
mod database;
mod features;
mod helpers;
mod middlewares;
mod observability;
mod routes;

use secrecy::ExposeSecret;

use crate::config::Config;

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    dotenv().ok();

    let config = Config::from_env();
    let observability = observability::init(&config.observability)
        .expect("failed to initialize configured OTLP exporters");

    let pool = database::setup_database(
        config.database_url.expose_secret(),
        config.observability.slow_query_threshold,
    )
    .await;

    let app = routes::with_routes(Route::new())
        .with(AddData::new(pool.clone()))
        .with(AddData::new(config.clone()))
        .with(middlewares::HttpObservability::new());

    let host = format!("0.0.0.0:{}", config.port);
    tracing::info!(%host, "server listening");

    let worker = background::start(pool.clone());

    Server::new(TcpListener::bind(host))
        .run_with_graceful_shutdown(app, shutdown_signal(), Some(Duration::from_secs(10)))
        .await?;

    tracing::info!("server stopped; aborting background workers");
    worker.abort();
    let _ = worker.await;

    observability.shutdown();
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }

    tracing::info!("signal received; starting graceful shutdown");
}
