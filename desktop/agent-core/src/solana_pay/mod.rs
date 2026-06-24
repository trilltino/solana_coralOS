pub mod monitor;
pub mod payment;
pub mod strategies;
pub mod url;
pub mod validation;

pub use monitor::TritonPaymentMonitorStrategy;
pub use payment::{demo_payment_flow, parse_402_response, PaymentChallenge, PaymentProtocol, DemoPaymentResult};
pub use strategies::{PaymentStrategy, TransferStrategy, WeatherStrategy};
pub use url::{encode_transfer_url, encode_transaction_request_url, parse_url, ParsedUrl, TransferUrlFields, TransactionRequestUrlFields};
pub use validation::{validate_transfer, ValidationResult};

pub fn get_transfer_capabilities() -> Vec<String> {
    vec![
        "encode_transfer_url".to_string(),
        "encode_transaction_request_url".to_string(),
        "parse_url".to_string(),
        "validate_transfer".to_string(),
    ]
}

pub fn get_payment_capabilities() -> Vec<String> {
    vec![
        "parse_402_response".to_string(),
        "demo_payment_flow".to_string(),
        "validate_transfer".to_string(),
    ]
}
