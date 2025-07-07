use actix_web::{web, App, HttpServer};
use payment_engine::{routes, state::AppState};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info"),
    )
    .init();

    let shared_state = web::Data::new(AppState::new());

    HttpServer::new(move || {
        App::new()
            .app_data(shared_state.clone())
            .configure(routes::configure)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
