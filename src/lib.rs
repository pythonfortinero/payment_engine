pub mod config;
pub mod error;
pub mod events;
pub mod metrics;
pub mod models;
pub mod repository;
pub mod routes;
pub mod state;

use actix_web::{
    App, HttpMessage,
    dev::Service,
    http::header::{HeaderName, HeaderValue},
    middleware, web,
};
use std::time::Instant;
use tracing_actix_web::TracingLogger;
use uuid::Uuid;

use crate::{error::ApiError, state::AppState};

pub const CORRELATION_HEADER: &str = "x-correlation-id";

pub fn create_app(
    state: web::Data<AppState>,
) -> App<
    impl actix_web::dev::ServiceFactory<
        actix_web::dev::ServiceRequest,
        Config = (),
        Response = actix_web::dev::ServiceResponse<impl actix_web::body::MessageBody>,
        Error = actix_web::Error,
        InitError = (),
    >,
> {
    App::new()
        .app_data(state)
        .app_data(
            web::JsonConfig::default()
                .limit(16_384)
                .error_handler(|err, req| {
                    let correlation_id = req
                        .extensions()
                        .get::<String>()
                        .cloned()
                        .unwrap_or_else(|| Uuid::new_v4().to_string());
                    ApiError::bad_request("invalid_json", format!("Invalid JSON body: {err}"))
                        .with_correlation_id(correlation_id)
                        .into()
                }),
        )
        .app_data(web::PathConfig::default().error_handler(|err, req| {
            let correlation_id = req
                .extensions()
                .get::<String>()
                .cloned()
                .unwrap_or_else(|| Uuid::new_v4().to_string());
            ApiError::bad_request("invalid_path", format!("Invalid path parameter: {err}"))
                .with_correlation_id(correlation_id)
                .into()
        }))
        .wrap(TracingLogger::default())
        .wrap_fn(|req, srv| {
            let started = Instant::now();
            let state = req.app_data::<web::Data<AppState>>().cloned();
            let correlation_id = req
                .headers()
                .get(CORRELATION_HEADER)
                .and_then(|value| value.to_str().ok())
                .filter(|value| !value.trim().is_empty() && value.len() <= 128)
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| Uuid::new_v4().to_string());
            req.extensions_mut().insert(correlation_id.clone());
            let fut = srv.call(req);
            async move {
                let mut response = fut.await?;
                if let Ok(value) = HeaderValue::from_str(&correlation_id) {
                    response
                        .headers_mut()
                        .insert(HeaderName::from_static(CORRELATION_HEADER), value);
                }
                if let Some(state) = state {
                    let route = response
                        .request()
                        .match_pattern()
                        .unwrap_or_else(|| "unmatched".to_owned());
                    let status = response.status().as_u16().to_string();
                    state
                        .metrics
                        .observe_http(&route, &status, started.elapsed());
                }
                Ok(response)
            }
        })
        .wrap(middleware::NormalizePath::trim())
        .configure(routes::configure)
}
