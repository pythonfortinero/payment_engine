use actix_web::{HttpServer, web};
use payment_engine::{config::Config, create_app, events, metrics::Metrics, state::AppState};
use sqlx::postgres::PgPoolOptions;
use tokio::{sync::mpsc, time::timeout};
use tracing_subscriber::EnvFilter;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn")),
        )
        .with_current_span(true)
        .init();

    let config = Config::from_env()
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?;
    let pool = PgPoolOptions::new()
        .max_connections(config.max_connections)
        .acquire_timeout(config.database_timeout)
        .connect(&config.database_url)
        .await
        .map_err(std::io::Error::other)?;
    sqlx::migrate!()
        .run(&pool)
        .await
        .map_err(std::io::Error::other)?;

    let metrics = Metrics::new();
    let (event_sender, event_receiver) = mpsc::channel(config.event_buffer);
    let worker = events::spawn_worker(event_receiver, metrics.clone());
    let state = web::Data::new(AppState::new(pool.clone(), event_sender, metrics));
    let bind_address = (config.host, config.port);
    tracing::info!(host = %config.host, port = config.port, "starting payment engine");

    HttpServer::new(move || create_app(state.clone()))
        .client_request_timeout(config.request_timeout)
        .shutdown_timeout(config.shutdown_timeout.as_secs())
        .bind(bind_address)?
        .run()
        .await?;

    pool.close().await;
    if timeout(config.shutdown_timeout, worker).await.is_err() {
        tracing::warn!("event worker did not stop before the shutdown deadline");
    }
    Ok(())
}
