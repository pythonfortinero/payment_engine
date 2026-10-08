use actix_web::{HttpMessage, HttpRequest, HttpResponse, web};
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::{
    error::ApiError,
    events::TransactionCommitted,
    models::{CreditRequest, DebitRequest, NewClientRequest, NewTransaction, TransactionKind},
    repository,
    state::AppState,
};

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/health", web::get().to(health))
        .route("/ready", web::get().to(ready))
        .route("/metrics", web::get().to(metrics))
        .route("/new_client", web::post().to(new_client))
        .route("/new_credit_transaction", web::post().to(new_credit))
        .route("/new_debit_transaction", web::post().to(new_debit))
        .route("/client_balance/{client_id}", web::get().to(client_balance))
        .default_service(web::route().to(not_found_route));
}

async fn not_found_route(req: HttpRequest) -> Result<HttpResponse, ApiError> {
    Err(correlated(
        ApiError::new(
            actix_web::http::StatusCode::NOT_FOUND,
            "route_not_found",
            "Route not found",
        ),
        &req,
    ))
}

async fn health() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({"status": "healthy"}))
}

async fn ready(data: web::Data<AppState>) -> HttpResponse {
    if repository::is_ready(&data.pool).await {
        HttpResponse::Ok().json(serde_json::json!({"status": "ready"}))
    } else {
        HttpResponse::ServiceUnavailable().json(serde_json::json!({"status": "not_ready"}))
    }
}

async fn metrics(data: web::Data<AppState>) -> Result<HttpResponse, ApiError> {
    let body = data.metrics.encode().map_err(|error| {
        tracing::error!(%error, "failed to encode metrics");
        ApiError::internal("Could not encode metrics")
    })?;
    Ok(HttpResponse::Ok()
        .content_type("text/plain; version=0.0.4; charset=utf-8")
        .body(body))
}

async fn new_client(
    req: HttpRequest,
    data: web::Data<AppState>,
    payload: web::Json<NewClientRequest>,
) -> Result<HttpResponse, ApiError> {
    validate_client(&payload).map_err(|error| correlated(error, &req))?;
    repository::create_client(&data.pool, &payload)
        .await
        .map(|client| HttpResponse::Created().json(client))
        .map_err(|error| correlated(error, &req))
}

async fn new_credit(
    req: HttpRequest,
    data: web::Data<AppState>,
    payload: web::Json<CreditRequest>,
) -> Result<HttpResponse, ApiError> {
    transact(
        &req,
        &data,
        payload.client_id,
        payload.credit_amount,
        TransactionKind::Credit,
    )
    .await
}

async fn new_debit(
    req: HttpRequest,
    data: web::Data<AppState>,
    payload: web::Json<DebitRequest>,
) -> Result<HttpResponse, ApiError> {
    transact(
        &req,
        &data,
        payload.client_id,
        payload.debit_amount,
        TransactionKind::Debit,
    )
    .await
}

async fn transact(
    req: &HttpRequest,
    data: &web::Data<AppState>,
    client_id: Uuid,
    amount: Decimal,
    kind: TransactionKind,
) -> Result<HttpResponse, ApiError> {
    validate_amount(amount).map_err(|error| correlated(error, req))?;
    let key = idempotency_key(req).map_err(|error| correlated(error, req))?;
    let result = repository::create_transaction(
        &data.pool,
        NewTransaction {
            client_id,
            kind,
            amount,
            idempotency_key: &key,
        },
    )
    .await
    .map_err(|error| correlated(error, req))?;
    data.metrics.transaction(kind.as_str(), result.replayed);
    if !result.replayed {
        let event = TransactionCommitted {
            transaction_id: result.transaction.id,
            client_id,
            kind,
            amount,
        };
        if let Err(error) = data.events.try_send(event) {
            data.metrics.event("dropped");
            tracing::warn!(%error, "event queue unavailable; transaction remains committed");
        }
    }
    Ok(HttpResponse::Ok().json(result))
}

async fn client_balance(
    req: HttpRequest,
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    repository::get_balance(&data.pool, path.into_inner())
        .await
        .map(|balance| HttpResponse::Ok().json(balance))
        .map_err(|error| correlated(error, &req))
}

fn validate_amount(amount: Decimal) -> Result<(), ApiError> {
    if amount <= Decimal::ZERO {
        Err(ApiError::bad_request(
            "invalid_amount",
            "Amount must be greater than zero",
        ))
    } else {
        Ok(())
    }
}

fn validate_client(request: &NewClientRequest) -> Result<(), ApiError> {
    if request.client_name.trim().is_empty() || request.document_number.trim().is_empty() {
        return Err(ApiError::bad_request(
            "invalid_client",
            "Client name and document number are required",
        ));
    }
    if request.country.trim().len() != 2 {
        return Err(ApiError::bad_request(
            "invalid_country",
            "Country must be a two-letter code",
        ));
    }
    Ok(())
}

fn idempotency_key(req: &HttpRequest) -> Result<String, ApiError> {
    let value = req
        .headers()
        .get("idempotency-key")
        .ok_or_else(|| {
            ApiError::bad_request(
                "missing_idempotency_key",
                "Idempotency-Key header is required",
            )
        })?
        .to_str()
        .map_err(|_| {
            ApiError::bad_request(
                "invalid_idempotency_key",
                "Idempotency-Key must be valid ASCII",
            )
        })?;
    if value.trim().is_empty() || value.len() > 128 || value.chars().any(char::is_control) {
        return Err(ApiError::bad_request(
            "invalid_idempotency_key",
            "Idempotency-Key must contain 1 to 128 printable characters",
        ));
    }
    Ok(value.to_owned())
}

fn correlated(error: ApiError, req: &HttpRequest) -> ApiError {
    let id = req
        .extensions()
        .get::<String>()
        .cloned()
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    error.with_correlation_id(id)
}
