use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Client {
    pub id: Uuid,
    pub client_name: String,
    pub birth_date: NaiveDate,
    pub document_number: String,
    pub country: String,
    pub balance: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewClientRequest {
    pub client_name: String,
    pub birth_date: NaiveDate,
    pub document_number: String,
    pub country: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditRequest {
    pub client_id: Uuid,
    pub credit_amount: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebitRequest {
    pub client_id: Uuid,
    pub debit_amount: Decimal,
}

