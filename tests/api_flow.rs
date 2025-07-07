use actix_web::{test, web, App};
use payment_engine::{
    models::{NewClientRequest, DebitRequest, CreditRequest, Client},
    routes::configure,
    state::AppState,
};
use rust_decimal::Decimal;
use uuid::Uuid;
use chrono::NaiveDate;

#[actix_web::test]
async fn end_to_end_happy_path() {
    // State and app
    let state = web::Data::new(AppState::new());
    let app = test::init_service(
        App::new().app_data(state.clone()).configure(configure),
    )
    .await;

    // -------- 1. Create client --------
    let req_body = NewClientRequest {
        client_name: "Alice".into(),
        birth_date: NaiveDate::from_ymd_opt(1990, 2, 15).unwrap(),
        document_number: "12345678".into(),
        country: "AR".into(),
    };

    let client: Client = test::call_and_read_body_json::<_, _, Client>(
        &app,
        test::TestRequest::post()
            .uri("/new_client")
            .set_json(&req_body)
            .to_request(),
    )
    .await;

    let client_id: Uuid = client.id;

    // -------- 2. Credit ----------
    let credit = CreditRequest { client_id, credit_amount: Decimal::new(1_000, 2) };
    let balance: Decimal = test::call_and_read_body_json::<_, _, Decimal>(
        &app,
        test::TestRequest::post()
            .uri("/new_credit_transaction")
            .set_json(&credit)
            .to_request(),
    )
    .await;
    assert_eq!(balance, Decimal::new(1_000, 2));

    // -------- 3. Debit ----------
    let debit  = DebitRequest  { client_id, debit_amount:  Decimal::new(500, 2) };
    let balance: Decimal = test::call_and_read_body_json::<_, _, Decimal>(
        &app,
        test::TestRequest::post()
            .uri("/new_debit_transaction")
            .set_json(&debit)
            .to_request(),
    )
    .await;
    assert_eq!(balance, Decimal::new(500, 2));

    // -------- 4. Balance requests ----------
    let fetched: Client = test::call_and_read_body_json::<_, _, Client>(
        &app,
        test::TestRequest::get()
            .uri(&format!("/client_balance/{client_id}"))
            .to_request(),
    )
    .await;
    assert_eq!(fetched.balance, Decimal::new(500, 2));
}
