use actix_web::{web, HttpResponse, Responder, Result};
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::{
    models::{Client, CreditRequest, DebitRequest, NewClientRequest},
    state::AppState,
    storage,
};

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("")
            .route("/new_client", web::post().to(new_client))
            .route("/new_credit_transaction", web::post().to(new_credit))
            .route("/new_debit_transaction", web::post().to(new_debit))
            .route("/client_balance/{client_id}", web::get().to(client_balance))
            .route("/store_balances", web::post().to(store_balances)),
    );
}

async fn new_client(
    data: web::Data<AppState>,
    payload: web::Json<NewClientRequest>,
) -> impl Responder {
    let mut clients = data.clients.lock().await;

    if clients
        .values()
        .any(|c| c.document_number == payload.document_number)
    {
        return HttpResponse::BadRequest().body("Document number already exists");
    }

    let client = Client {
        id: Uuid::new_v4(),
        client_name: payload.client_name.clone(),
        birth_date: payload.birth_date,
        document_number: payload.document_number.clone(),
        country: payload.country.clone(),
        balance: Decimal::ZERO,
    };

    clients.insert(client.id, client.clone());

    HttpResponse::Ok().json(client)
}

async fn new_credit(
    data: web::Data<AppState>,
    payload: web::Json<CreditRequest>,
) -> impl Responder {
    adjust_balance(
        data,
        payload.client_id,
        payload.credit_amount,
        true,
    )
    .await
}

async fn new_debit(
    data: web::Data<AppState>,
    payload: web::Json<DebitRequest>,
) -> impl Responder {
    adjust_balance(
        data,
        payload.client_id,
        payload.debit_amount,
        false
    )
    .await
}

async fn adjust_balance(
    data: web::Data<AppState>,
    client_id: Uuid,
    amount: Decimal,
    is_credit: bool,
) -> HttpResponse {
    let mut clients = data.clients.lock().await;

    let client = match clients.get_mut(&client_id) {
        Some(c) => c,
        None => return HttpResponse::NotFound().body("Client not found"),
    };

    if is_credit {
        client.balance += amount;
    } else {
        if client.balance < amount {
            return HttpResponse::BadRequest().body("Insufficient funds");
        }
        client.balance -= amount;
    }

    HttpResponse::Ok().json(client.balance)
}

async fn client_balance(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> impl Responder {
    let clients = data.clients.lock().await;
    let client_id = path.into_inner();

    if let Some(client) = clients.get(&client_id) {
        HttpResponse::Ok().json(client)
    } else {
        HttpResponse::NotFound().body("Client not found")
    }
}

async fn store_balances(data: web::Data<AppState>) -> Result<impl Responder> {
    let mut clients = data.clients.lock().await;
    let mut counter = data.file_counter.lock().await;

    let filename = storage::persist(&clients, *counter).await.map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Persist failed: {e}"))
    })?;

    *counter += 1;

    for client in clients.values_mut() {
        client.balance = Decimal::ZERO;
    }

    Ok(HttpResponse::Ok().body(format!("Balances stored in {filename}")))
}