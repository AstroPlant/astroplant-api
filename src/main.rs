#[macro_use]
extern crate log;

#[macro_use]
extern crate diesel;

#[macro_use]
extern crate validator_derive;

#[macro_use]
extern crate strum_macros;

use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use once_cell::sync::OnceCell;
use warp::{self, http::Method, path, Filter, Rejection, Reply};

type PgPool = Pool<ConnectionManager<PgConnection>>;
type PgPooled = PooledConnection<ConnectionManager<PgConnection>>;

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

use response::{Response, ResponseBuilder};

static VERSION: &str = env!("CARGO_PKG_VERSION");
static DEFAULT_DATABASE_URL: &str = "postgres://astroplant:astroplant@localhost/astroplant";
static DEFAULT_MQTT_HOST: &str = "mqtt.ops";
const DEFAULT_MQTT_PORT: u16 = 1883;
static DEFAULT_MQTT_USERNAME: &str = "server";
static DEFAULT_MQTT_PASSWORD: &str = "";

static TOKEN_SIGNER: OnceCell<astroplant_auth::token::TokenSigner> = OnceCell::new();

fn pg_pool() -> PgPool {
    let manager = ConnectionManager::<PgConnection>::new(
        std::env::var("DATABASE_URL").unwrap_or(DEFAULT_DATABASE_URL.to_owned()),
    );
    Pool::builder()
        .connection_timeout(std::time::Duration::from_secs(5))
        .build(manager)
        .expect("PostgreSQL connection pool could not be created.")
}

#[tokio::main]
async fn main() {
    env_logger::init();

    init_token_signer();

    let pg_pool = pg_pool();

    // Start MQTT.
    let (raw_measurement_receiver, kits_rpc) = mqtt::run(pg_pool.clone());

    // Start WebSockets.
    let (ws_endpoint, publisher) = astroplant_websocket::run();
    tokio::runtime::Handle::current().spawn(websocket::run(publisher, raw_measurement_receiver));

    let rate_limit = rate_limit::leaky_bucket();
    let pg = helpers::pg(pg_pool);

    let rest_endpoints = (path!("version")
        .map(|| ResponseBuilder::ok().body(VERSION))
        .or(path!("time")
            .map(|| ResponseBuilder::ok().body(chrono::Utc::now().to_rfc3339()))
            .boxed())
        .unify()
        .or(path!("kits" / ..).and(controllers::kit::router(pg.clone().boxed())))
        .unify()
        .or(path!("kit-configurations" / ..)
            .and(controllers::kit_configuration::router(pg.clone().boxed())))
        .unify()
        .or(path!("kit-rpc" / ..).and(controllers::kit_rpc::router(kits_rpc, pg.clone().boxed())))
        .unify()
        .or(path!("users" / ..).and(controllers::user::router(pg.clone().boxed())))
        .unify()
        .or(path!("me" / ..).and(controllers::me::router(pg.clone().boxed())))
        .unify()
        .or(
            path!("peripheral-definitions" / ..).and(controllers::peripheral_definition::router(
                pg.clone().boxed(),
            )),
        )
        .unify()
        .or(path!("quantity-types" / ..)
            .and(controllers::quantity_type::router(pg.clone().boxed())))
        .unify()
        .or(path!("permissions" / ..).and(controllers::permission::router(pg.clone().boxed())))
        .unify()
        .or(path!("measurements" / ..).and(controllers::measurement::router(pg.clone().boxed())))
        .unify())
    .and(warp::header("Accept"))
    .map(|response: Response, _accept: String| {
        // TODO: utilize Accept header, e.g. returning XML when requested.

        let mut http_response_builder = warp::http::response::Builder::new()
            .status(response.status_code())
            .header("Content-Type", "application/json");

        for (header, value) in response.headers() {
            http_response_builder = http_response_builder.header(header.as_bytes(), value.clone());
        }

        match response.value() {
            Some(value) => http_response_builder
                .body(serde_json::to_string(value).unwrap())
                .unwrap(),
            None => http_response_builder.body("".to_owned()).unwrap(),
        }
    })
    .recover(|rejection| async { handle_rejection(rejection) })
    .with(warp::log("astroplant_rs_api::api"))
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
            .allow_headers(vec!["Authorization", "Content-Type"]),
    );

    let all = rate_limit.and(ws_endpoint.or(rest_endpoints));

    warp::serve(all).run(([0, 0, 0, 0], 8080)).await;
}

/// Convert rejections into replies.
fn handle_rejection(rejection: Rejection) -> Result<impl Reply, Rejection> {
    use problem::{DescriptiveProblem, Problem};

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
    debug!("Using token signer key file {}", key_file_path);

    let token_signer_key: Vec<u8> = std::fs::read(&key_file_path).unwrap();
    trace!(
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
