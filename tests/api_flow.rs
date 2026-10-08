use actix_web::{http::StatusCode, test, web};
use payment_engine::{
    create_app,
    events::TransactionCommitted,
    metrics::Metrics,
    models::{
        BalanceResponse, CreditRequest, DebitRequest, NewClientRequest, NewTransaction,
        TransactionKind,
    },
    repository,
    state::AppState,
};
use rust_decimal::Decimal;
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::{env, str::FromStr, time::Duration};
use tokio::sync::mpsc;
use uuid::Uuid;

async fn pool() -> PgPool {
    let url = env::var("TEST_DATABASE_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .expect("TEST_DATABASE_URL or DATABASE_URL is required for integration tests");
    let pool = PgPoolOptions::new()
        .max_connections(20)
        .connect(&url)
        .await
        .expect("connect test database");
    sqlx::migrate!().run(&pool).await.expect("run migrations");
    pool
}

fn request(document: String) -> NewClientRequest {
    NewClientRequest {
        client_name: "Test Client".into(),
        birth_date: chrono::NaiveDate::from_ymd_opt(1990, 2, 15).unwrap(),
        document_number: document,
        country: "AR".into(),
    }
}

async fn client(pool: &PgPool) -> payment_engine::models::Client {
    repository::create_client(pool, &request(Uuid::new_v4().to_string()))
        .await
        .unwrap()
}

async fn transaction(
    pool: &PgPool,
    client_id: Uuid,
    kind: TransactionKind,
    amount: Decimal,
    key: &str,
) -> payment_engine::models::TransactionResponse {
    repository::create_transaction(
        pool,
        NewTransaction {
            client_id,
            kind,
            amount,
            idempotency_key: key,
        },
    )
    .await
    .unwrap()
}

fn state(pool: PgPool) -> web::Data<AppState> {
    let (sender, _receiver) = mpsc::channel::<TransactionCommitted>(128);
    web::Data::new(AppState::new(pool, sender, Metrics::new()))
}

#[actix_web::test]
async fn rejects_negative_credits_and_debits_with_consistent_json() {
    let app = test::init_service(create_app(state(pool().await))).await;
    let id = Uuid::new_v4();
    for (path, body) in [
        (
            "/new_credit_transaction",
            serde_json::to_value(CreditRequest {
                client_id: id,
                credit_amount: Decimal::NEGATIVE_ONE,
            })
            .unwrap(),
        ),
        (
            "/new_debit_transaction",
            serde_json::to_value(DebitRequest {
                client_id: id,
                debit_amount: Decimal::ZERO,
            })
            .unwrap(),
        ),
    ] {
        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri(path)
                .insert_header(("Idempotency-Key", Uuid::new_v4().to_string()))
                .set_json(body)
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(response.headers().contains_key("x-correlation-id"));
        let json: serde_json::Value = test::read_body_json(response).await;
        assert_eq!(json["error"]["code"], "invalid_amount");
        assert!(json["error"]["correlation_id"].is_string());
    }
}

#[actix_web::test]
async fn rejects_insufficient_funds_and_missing_clients() {
    let pool = pool().await;
    let client = client(&pool).await;
    let error = repository::create_transaction(
        &pool,
        NewTransaction {
            client_id: client.id,
            kind: TransactionKind::Debit,
            amount: Decimal::ONE,
            idempotency_key: &Uuid::new_v4().to_string(),
        },
    )
    .await
    .unwrap_err();
    assert_eq!(
        actix_web::ResponseError::status_code(&error),
        StatusCode::BAD_REQUEST
    );

    let error = repository::create_transaction(
        &pool,
        NewTransaction {
            client_id: Uuid::new_v4(),
            kind: TransactionKind::Credit,
            amount: Decimal::ONE,
            idempotency_key: &Uuid::new_v4().to_string(),
        },
    )
    .await
    .unwrap_err();
    assert_eq!(
        actix_web::ResponseError::status_code(&error),
        StatusCode::NOT_FOUND
    );
}

#[actix_web::test]
async fn rejects_duplicate_documents() {
    let pool = pool().await;
    let request = request(Uuid::new_v4().to_string());
    repository::create_client(&pool, &request).await.unwrap();
    let error = repository::create_client(&pool, &request)
        .await
        .unwrap_err();
    assert_eq!(
        actix_web::ResponseError::status_code(&error),
        StatusCode::CONFLICT
    );
}

#[actix_web::test]
async fn repeated_idempotency_key_returns_the_original_transaction() {
    let pool = pool().await;
    let client = client(&pool).await;
    let key = Uuid::new_v4().to_string();
    let first = transaction(
        &pool,
        client.id,
        TransactionKind::Credit,
        Decimal::from(25),
        &key,
    )
    .await;
    let replay = transaction(
        &pool,
        client.id,
        TransactionKind::Credit,
        Decimal::from(25),
        &key,
    )
    .await;
    assert_eq!(first.transaction.id, replay.transaction.id);
    assert!(!first.replayed);
    assert!(replay.replayed);
    assert_eq!(replay.balance, Decimal::from(25));
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM ledger_transactions WHERE idempotency_key = $1",
    )
    .bind(key)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1);
}

#[actix_web::test]
async fn concurrent_account_operations_are_serialized_safely() {
    let pool = pool().await;
    let client = client(&pool).await;
    transaction(
        &pool,
        client.id,
        TransactionKind::Credit,
        Decimal::from(1000),
        &Uuid::new_v4().to_string(),
    )
    .await;
    let mut tasks = Vec::new();
    for _ in 0..50 {
        for (kind, amount) in [
            (TransactionKind::Credit, Decimal::from(3)),
            (TransactionKind::Debit, Decimal::ONE),
        ] {
            let pool = pool.clone();
            let key = Uuid::new_v4().to_string();
            let client_id = client.id;
            tasks.push(tokio::spawn(async move {
                repository::create_transaction(
                    &pool,
                    NewTransaction {
                        client_id,
                        kind,
                        amount,
                        idempotency_key: &key,
                    },
                )
                .await
            }));
        }
    }
    for task in tasks {
        task.await.unwrap().unwrap();
    }
    let balance = repository::get_balance(&pool, client.id).await.unwrap();
    assert_eq!(balance.balance, Decimal::from(1100));
}

#[actix_web::test]
async fn balance_is_derived_from_the_immutable_ledger() {
    let pool = pool().await;
    let client = client(&pool).await;
    transaction(
        &pool,
        client.id,
        TransactionKind::Credit,
        Decimal::from_str("100.25").unwrap(),
        &Uuid::new_v4().to_string(),
    )
    .await;
    transaction(
        &pool,
        client.id,
        TransactionKind::Debit,
        Decimal::from_str("40.10").unwrap(),
        &Uuid::new_v4().to_string(),
    )
    .await;
    transaction(
        &pool,
        client.id,
        TransactionKind::Credit,
        Decimal::from_str("2.35").unwrap(),
        &Uuid::new_v4().to_string(),
    )
    .await;
    let BalanceResponse { balance, .. } = repository::get_balance(&pool, client.id).await.unwrap();
    assert_eq!(balance, Decimal::from_str("62.50").unwrap());
    let update = sqlx::query("UPDATE ledger_transactions SET amount = 1 WHERE client_id = $1")
        .bind(client.id)
        .execute(&pool)
        .await;
    assert!(update.is_err(), "ledger rows must be immutable");
}

#[actix_web::test]
async fn persistence_failure_returns_service_unavailable_json() {
    let pool = PgPoolOptions::new()
        .acquire_timeout(Duration::from_millis(100))
        .connect_lazy("postgres://nobody:nothing@127.0.0.1:1/unavailable")
        .unwrap();
    let app = test::init_service(create_app(state(pool))).await;
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/new_client")
            .set_json(request(Uuid::new_v4().to_string()))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let json: serde_json::Value = test::read_body_json(response).await;
    assert_eq!(json["error"]["code"], "persistence_unavailable");
}
