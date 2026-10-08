use crate::{
    error::ApiError,
    models::{
        BalanceResponse, Client, LedgerTransaction, NewClientRequest, NewTransaction,
        TransactionKind, TransactionResponse,
    },
};
use rust_decimal::Decimal;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

pub async fn create_client(pool: &PgPool, request: &NewClientRequest) -> Result<Client, ApiError> {
    let result = sqlx::query_as::<_, Client>(r#"INSERT INTO clients (id, client_name, birth_date, document_number, country) VALUES ($1, $2, $3, $4, $5) RETURNING id, client_name, birth_date, document_number, country, created_at"#)
        .bind(Uuid::new_v4()).bind(request.client_name.trim()).bind(request.birth_date)
        .bind(request.document_number.trim()).bind(request.country.trim().to_uppercase()).fetch_one(pool).await;
    match result {
        Ok(client) => Ok(client),
        Err(sqlx::Error::Database(error))
            if error.constraint() == Some("clients_document_number_key") =>
        {
            Err(ApiError::conflict(
                "duplicate_document",
                "A client with this document number already exists",
            ))
        }
        Err(error) => Err(ApiError::from_sqlx(error)),
    }
}

pub async fn create_transaction(
    pool: &PgPool,
    request: NewTransaction<'_>,
) -> Result<TransactionResponse, ApiError> {
    let mut tx = pool.begin().await.map_err(ApiError::from_sqlx)?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(request.idempotency_key)
        .execute(&mut *tx)
        .await
        .map_err(ApiError::from_sqlx)?;
    if let Some(existing) = find_by_idempotency_key(&mut tx, request.idempotency_key).await? {
        if existing.client_id != request.client_id
            || existing.kind != request.kind
            || existing.amount != request.amount
        {
            return Err(ApiError::conflict(
                "idempotency_conflict",
                "The idempotency key was already used with a different operation",
            ));
        }
        let balance = balance_in_transaction(&mut tx, request.client_id).await?;
        tx.commit().await.map_err(ApiError::from_sqlx)?;
        return Ok(TransactionResponse {
            transaction: existing,
            balance,
            replayed: true,
        });
    }
    let client_exists =
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM clients WHERE id = $1 FOR UPDATE")
            .bind(request.client_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(ApiError::from_sqlx)?
            .is_some();
    if !client_exists {
        return Err(ApiError::not_found("Client not found"));
    }
    let balance = balance_in_transaction(&mut tx, request.client_id).await?;
    if request.kind == TransactionKind::Debit && balance < request.amount {
        return Err(ApiError::bad_request(
            "insufficient_funds",
            "Insufficient funds",
        ));
    }
    let transaction = sqlx::query_as::<_, LedgerTransaction>(r#"INSERT INTO ledger_transactions (id, client_id, kind, amount, idempotency_key) VALUES ($1, $2, $3, $4, $5) RETURNING id, client_id, kind, amount, idempotency_key, created_at"#)
        .bind(Uuid::new_v4()).bind(request.client_id).bind(request.kind).bind(request.amount).bind(request.idempotency_key).fetch_one(&mut *tx).await.map_err(ApiError::from_sqlx)?;
    let new_balance = match request.kind {
        TransactionKind::Credit => balance + request.amount,
        TransactionKind::Debit => balance - request.amount,
    };
    tx.commit().await.map_err(ApiError::from_sqlx)?;
    Ok(TransactionResponse {
        transaction,
        balance: new_balance,
        replayed: false,
    })
}

async fn find_by_idempotency_key(
    tx: &mut Transaction<'_, Postgres>,
    key: &str,
) -> Result<Option<LedgerTransaction>, ApiError> {
    sqlx::query_as::<_, LedgerTransaction>("SELECT id, client_id, kind, amount, idempotency_key, created_at FROM ledger_transactions WHERE idempotency_key = $1")
        .bind(key).fetch_optional(&mut **tx).await.map_err(ApiError::from_sqlx)
}

async fn balance_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    client_id: Uuid,
) -> Result<Decimal, ApiError> {
    sqlx::query_scalar::<_, Decimal>("SELECT COALESCE(SUM(CASE WHEN kind = 'credit' THEN amount ELSE -amount END), 0)::NUMERIC FROM ledger_transactions WHERE client_id = $1")
        .bind(client_id).fetch_one(&mut **tx).await.map_err(ApiError::from_sqlx)
}

pub async fn get_balance(pool: &PgPool, client_id: Uuid) -> Result<BalanceResponse, ApiError> {
    let client = sqlx::query_as::<_, Client>("SELECT id, client_name, birth_date, document_number, country, created_at FROM clients WHERE id = $1")
        .bind(client_id).fetch_optional(pool).await.map_err(ApiError::from_sqlx)?.ok_or_else(|| ApiError::not_found("Client not found"))?;
    let balance = sqlx::query_scalar::<_, Decimal>("SELECT COALESCE(SUM(CASE WHEN kind = 'credit' THEN amount ELSE -amount END), 0)::NUMERIC FROM ledger_transactions WHERE client_id = $1")
        .bind(client_id).fetch_one(pool).await.map_err(ApiError::from_sqlx)?;
    Ok(BalanceResponse { client, balance })
}

pub async fn is_ready(pool: &PgPool) -> bool {
    sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(pool)
        .await
        .is_ok()
}
