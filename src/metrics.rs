use prometheus::{
    Encoder, HistogramOpts, HistogramVec, IntCounterVec, Opts, Registry, TextEncoder,
};
use std::time::Duration;

#[derive(Clone)]
pub struct Metrics {
    registry: Registry,
    http_duration: HistogramVec,
    http_errors: IntCounterVec,
    transactions: IntCounterVec,
    events: IntCounterVec,
}

impl Metrics {
    pub fn new() -> Self {
        let registry = Registry::new();
        let http_duration = HistogramVec::new(
            HistogramOpts::new("http_request_duration_seconds", "HTTP request latency"),
            &["route", "status"],
        )
        .expect("valid metric");
        let http_errors = IntCounterVec::new(
            Opts::new("http_errors_total", "HTTP error responses"),
            &["route", "status"],
        )
        .expect("valid metric");
        let transactions = IntCounterVec::new(
            Opts::new("ledger_transactions_total", "Committed ledger transactions"),
            &["kind", "replayed"],
        )
        .expect("valid metric");
        let events = IntCounterVec::new(
            Opts::new("worker_events_total", "Asynchronous worker events"),
            &["result"],
        )
        .expect("valid metric");
        registry
            .register(Box::new(http_duration.clone()))
            .expect("unique metric");
        registry
            .register(Box::new(http_errors.clone()))
            .expect("unique metric");
        registry
            .register(Box::new(transactions.clone()))
            .expect("unique metric");
        registry
            .register(Box::new(events.clone()))
            .expect("unique metric");
        Self {
            registry,
            http_duration,
            http_errors,
            transactions,
            events,
        }
    }
    pub fn observe_http(&self, route: &str, status: &str, duration: Duration) {
        self.http_duration
            .with_label_values(&[route, status])
            .observe(duration.as_secs_f64());
        if status.starts_with('4') || status.starts_with('5') {
            self.http_errors.with_label_values(&[route, status]).inc();
        }
    }
    pub fn transaction(&self, kind: &str, replayed: bool) {
        self.transactions
            .with_label_values(&[kind, if replayed { "true" } else { "false" }])
            .inc();
    }
    pub fn event(&self, result: &str) {
        self.events.with_label_values(&[result]).inc();
    }
    pub fn encode(&self) -> Result<Vec<u8>, prometheus::Error> {
        let mut output = Vec::new();
        TextEncoder::new().encode(&self.registry.gather(), &mut output)?;
        Ok(output)
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}
