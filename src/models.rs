use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Client {
    pub id: Uuid,
    pub client_name: String,
    pub birth_date: NaiveDate,
    pub document_number: String,
    pub country: String,
    pub created_at: DateTime<Utc>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "transaction_kind", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum TransactionKind {
    Credit,
    Debit,
}

impl TransactionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Credit => "credit",
            Self::Debit => "debit",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LedgerTransaction {
    pub id: Uuid,
    pub client_id: Uuid,
    pub kind: TransactionKind,
    pub amount: Decimal,
    pub idempotency_key: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionResponse {
    pub transaction: LedgerTransaction,
    pub balance: Decimal,
    pub replayed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceResponse {
    #[serde(flatten)]
    pub client: Client,
    pub balance: Decimal,
}

#[derive(Debug, Clone)]
pub struct NewTransaction<'a> {
    pub client_id: Uuid,
    pub kind: TransactionKind,
    pub amount: Decimal,
    pub idempotency_key: &'a str,
}
