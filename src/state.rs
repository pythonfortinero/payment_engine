use crate::{events::TransactionCommitted, metrics::Metrics};
use sqlx::PgPool;
use tokio::sync::mpsc;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub events: mpsc::Sender<TransactionCommitted>,
    pub metrics: Metrics,
}

impl AppState {
    pub fn new(pool: PgPool, events: mpsc::Sender<TransactionCommitted>, metrics: Metrics) -> Self {
        Self {
            pool,
            events,
            metrics,
        }
    }
}
