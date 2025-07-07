use actix_web::error::ErrorBadRequest;
use rust_decimal::Decimal;

pub fn ensure_positive(amount: &Decimal) -> Result<(), actix_web::Error> {
    if amount.is_sign_positive() && !amount.is_zero() {
        Ok(())
    } else {
        Err(ErrorBadRequest("Amount must be greater than zero"))
    }
}