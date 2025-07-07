use crate::models::Client;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub clients: Arc<Mutex<HashMap<Uuid, Client>>>,
    pub file_counter: Arc<Mutex<u32>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            clients: Arc::new(Mutex::new(HashMap::new())),
            file_counter: Arc::new(Mutex::new(0)),
        }
    }
}