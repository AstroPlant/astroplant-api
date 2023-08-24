#[macro_use]
extern crate diesel;

#[macro_use]
extern crate strum_macros;

use once_cell::sync::OnceCell;

pub mod cursors;
pub mod database;
pub mod extract;
pub mod utils;

pub mod authorization;
pub mod helpers;
pub mod problem;
pub mod rate_limit;
pub mod schema;

pub mod controllers;
pub mod models;
pub mod response;
pub mod views;

pub mod mqtt;

static TOKEN_SIGNER: OnceCell<astroplant_auth::token::TokenSigner> = OnceCell::new();

pub static VERSION: &str = env!("CARGO_PKG_VERSION");
pub static DEFAULT_DATABASE_URL: &str = "postgres://astroplant:astroplant@localhost/astroplant";
pub static DEFAULT_MQTT_HOST: &str = "localhost";
pub const DEFAULT_MQTT_PORT: u16 = 1883;
pub static DEFAULT_S3_REGION: &str = "us-east-1";
pub static DEFAULT_S3_ENDPOINT: &str = "http://localhost:9000";

/// Initialize the token signer.
///
/// # Panics
/// This function is only callable once; it panics if called multiple times.
pub fn init_token_signer() {
    let key_file_path =
        std::env::var("TOKEN_SIGNER_KEY").unwrap_or("./token_signer.key".to_owned());
    tracing::debug!("Using token signer key file {}", key_file_path);

    let token_signer_key: Vec<u8> = std::fs::read(&key_file_path).unwrap();
    tracing::trace!(
        "Using token signer key of {} bits",
        token_signer_key.len() * 8
    );

    if TOKEN_SIGNER
        .set(astroplant_auth::token::TokenSigner::new(token_signer_key))
        .is_err()
    {
        panic!("Token signer initialization called more than once.")
    }
}
