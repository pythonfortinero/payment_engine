use crate::{metrics::Metrics, models::TransactionKind};
use rust_decimal::Decimal;
use tokio::{sync::mpsc, task::JoinHandle};
use uuid::Uuid;

#[derive(Debug)]
pub struct TransactionCommitted {
    pub transaction_id: Uuid,
    pub client_id: Uuid,
    pub kind: TransactionKind,
    pub amount: Decimal,
}

pub fn spawn_worker(
    mut receiver: mpsc::Receiver<TransactionCommitted>,
    metrics: Metrics,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(event) = receiver.recv().await {
            tracing::info!(transaction_id = %event.transaction_id, client_id = %event.client_id, kind = event.kind.as_str(), amount = %event.amount, "processed transaction event");
            metrics.event("processed");
        }
        tracing::info!("event worker stopped after draining its queue");
    })
}
