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
mod routes;

use crate::config::Config;

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    dotenv().ok();

    let config = Config::from_env();

    let pool = database::setup_database(&config.database_url).await;

    let app = routes::with_routes(Route::new())
        .with(AddData::new(pool.clone()))
        .with(AddData::new(config.clone()))
        .with(middlewares::BasicLog);

    let host = format!("0.0.0.0:{}", config.port);
    println!("Listening on http://{host}");

    let worker = background::start(pool.clone());

    Server::new(TcpListener::bind(host))
        .run_with_graceful_shutdown(app, shutdown_signal(), Some(Duration::from_secs(10)))
        .await?;

    println!("Server stopped, aborting background workers...");
    worker.abort();
    let _ = worker.await;

    println!("Server exiting.");
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

    println!("\nSignal received, starting graceful shutdown...");
}
