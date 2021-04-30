#[macro_use]
extern crate diesel;

#[macro_use]
extern crate strum_macros;

use once_cell::sync::OnceCell;
use warp::{self, http::Method, path, Filter, Rejection, Reply};

mod cursors;
mod database;
mod utils;

mod authentication;
mod authorization;
mod helpers;
mod problem;
mod rate_limit;
mod schema;

mod controllers;
mod models;
mod response;
mod views;

mod mqtt;
mod websocket;

use problem::{AppResult, DescriptiveProblem, Problem};
use response::{Response, ResponseBuilder, ResponseValue};

static VERSION: &str = env!("CARGO_PKG_VERSION");
static DEFAULT_DATABASE_URL: &str = "postgres://astroplant:astroplant@localhost/astroplant";
static DEFAULT_MQTT_HOST: &str = "localhost";
const DEFAULT_MQTT_PORT: u16 = 1883;
static DEFAULT_MQTT_USERNAME: &str = "server";
static DEFAULT_MQTT_PASSWORD: &str = "";
static DEFAULT_S3_REGION: &str = "us-east-1";
static DEFAULT_S3_ENDPOINT: &str = "http://localhost:9000";

static TOKEN_SIGNER: OnceCell<astroplant_auth::token::TokenSigner> = OnceCell::new();

#[tokio::main]
async fn main() {
    // env_logger::init();
    tracing_subscriber::fmt::init();

    init_token_signer();

    let pg = database::PgPool::new(
        std::env::var("DATABASE_URL").unwrap_or(DEFAULT_DATABASE_URL.to_owned()),
        std::time::Duration::from_secs(5),
    );

    let object_store = astroplant_object::ObjectStore::s3(
        std::env::var("AWS_S3_REGION").unwrap_or(DEFAULT_S3_REGION.to_owned()),
        std::env::var("AWS_S3_ENDPOINT").unwrap_or(DEFAULT_S3_ENDPOINT.to_owned()),
    );

    // Start MQTT.
    let (raw_measurement_receiver, kits_rpc) = mqtt::run(pg.clone(), object_store.clone());

    // Start WebSockets.
    let (ws_endpoint, publisher) = astroplant_websocket::run();
    tokio::runtime::Handle::current().spawn(websocket::run(publisher, raw_measurement_receiver));

    let rate_limit = rate_limit::leaky_bucket();

    let rest_endpoints = ((path!("version").map(|| Ok(ResponseBuilder::ok().body(VERSION))))
        .or(path!("time")
            .map(|| Ok(ResponseBuilder::ok().body(chrono::Utc::now().to_rfc3339())))
            .boxed())
        .unify()
        .or(path!("kits" / ..).and(controllers::kit::router(pg.clone())))
        .unify()
        .or(controllers::kit_configuration::router(pg.clone()))
        .unify()
        .or(path!("kit-rpc" / ..).and(controllers::kit_rpc::router(kits_rpc, pg.clone())))
        .unify()
        .or(path!("users" / ..).and(controllers::user::router(pg.clone())))
        .unify()
        .or(path!("me" / ..).and(controllers::me::router(pg.clone())))
        .unify()
        .or(path!("peripheral-definitions" / ..)
            .and(controllers::peripheral_definition::router(pg.clone())))
        .unify()
        .or(path!("quantity-types" / ..).and(controllers::quantity_type::router(pg.clone())))
        .unify()
        .or(path!("permissions" / ..).and(controllers::permission::router(pg.clone())))
        .unify()
        .or(controllers::measurement::router(pg.clone()))
        .unify()
        .or(controllers::media::router(pg.clone(), object_store.clone()))
        .unify())
    .and(warp::header("Accept"))
    .map(|response: AppResult<Response>, _accept: String| {
        // TODO: utilize Accept header, e.g. returning XML when requested.
        let mut http_response_builder = warp::http::response::Builder::new();
        match response {
            Ok(response) => {
                http_response_builder = http_response_builder.status(response.status_code());

                for (header, value) in response.headers() {
                    http_response_builder =
                        http_response_builder.header(header.as_bytes(), value.clone());
                }

                match response.value() {
                    Some(ResponseValue::Serializable(value)) => http_response_builder
                        .header("Content-Type", "application/json")
                        .body(warp::hyper::Body::from(serde_json::to_vec(&value).unwrap()))
                        .unwrap(),
                    Some(ResponseValue::Data { media_type, data }) => http_response_builder
                        .header("Content-Type", media_type)
                        .body(warp::hyper::Body::from(data))
                        // FIXME potentially dangerous unwrap
                        .unwrap(),
                    Some(ResponseValue::Stream {
                        media_type,
                        mut stream,
                    }) => {
                        use futures::stream::StreamExt;

                        let (mut sender, body) = warp::hyper::Body::channel();

                        tokio::spawn(async move {
                            while let Some(r) = stream.next().await {
                                match r {
                                    Ok(data) => {
                                        if let Err(_) = sender.send_data(data).await {
                                            break;
                                        }
                                    }
                                    Err(_) => {
                                        sender.abort();
                                        break;
                                    }
                                }
                            }
                        });

                        http_response_builder
                            .header("Content-Type", media_type)
                            .body(body)
                            // FIXME potentially dangerous unwrap
                            .unwrap()
                    }
                    None => http_response_builder
                        .body(warp::hyper::Body::empty())
                        .unwrap(),
                }
            }
            Err(problem) => {
                let descriptive_problem = DescriptiveProblem::from(&problem);

                http_response_builder
                    .status(problem.to_status_code())
                    .body(warp::hyper::Body::from(
                        serde_json::to_vec(&descriptive_problem).unwrap(),
                    ))
                    .unwrap()
            }
        }
    })
    .with(warp::log("astroplant_api::api"))
    // TODO: this wrapper might be better placed per-endpoint, to have accurate allowed metods
    .with(
        warp::cors()
            .allow_any_origin()
            .allow_methods(vec![
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::PATCH,
                Method::DELETE,
                Method::OPTIONS,
            ])
            .allow_headers(vec!["Content-Type", "Authorization"])
            .expose_headers(vec!["Link"])
            .build(),
    );

    let all = rate_limit
        .and(ws_endpoint.or(rest_endpoints))
        .recover(|rejection| async { handle_rejection(rejection) });

    warp::serve(all).run(([0, 0, 0, 0], 8080)).await;
}

/// Convert rejections into replies.
fn handle_rejection(rejection: Rejection) -> Result<impl Reply, Rejection> {
    let reply = if let Some(problem) = rejection.find::<Problem>() {
        // This rejection originated in this implementation.

        let descriptive_problem = DescriptiveProblem::from(problem);

        warp::reply::with_status(
            serde_json::to_string(&descriptive_problem).unwrap(),
            problem.to_status_code(),
        )
    } else {
        // This rejection originated in Warp.

        let problem = if rejection.is_not_found() {
            problem::NOT_FOUND
        } else if rejection.find::<warp::reject::InvalidQuery>().is_some() {
            problem::BAD_REQUEST
        } else {
            problem::INTERNAL_SERVER_ERROR
        };
        let descriptive_problem = DescriptiveProblem::from(&problem);

        warp::reply::with_status(
            serde_json::to_string(&descriptive_problem).unwrap(),
            problem.to_status_code(),
        )
    };

    Ok(warp::reply::with_header(
        reply,
        "Content-Type",
        "application/problem+json",
    ))
}

/// Initialize the token signer.
///
/// # Panics
/// This function is only callable once; it panics if called multiple times.
fn init_token_signer() {
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
