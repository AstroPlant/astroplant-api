#[macro_use]
extern crate log;

#[macro_use]
extern crate diesel;

#[macro_use]
extern crate validator_derive;

use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use once_cell::sync::OnceCell;
use warp::{self, http::Method, path, Filter, Rejection, Reply};

type PgPool = Pool<ConnectionManager<PgConnection>>;
type PgPooled = PooledConnection<ConnectionManager<PgConnection>>;

mod authentication;
mod helpers;
mod problem;
mod rate_limit;
mod schema;

mod controllers;
mod models;
mod response;
mod views;

use response::{Response, ResponseBuilder};

static VERSION: &str = "1.0.0-alpha";

static TOKEN_SIGNER: OnceCell<astroplant_auth::token::TokenSigner> = OnceCell::new();

fn pg_pool() -> PgPool {
    let manager = ConnectionManager::<PgConnection>::new(
        "postgres://astroplant:astroplant@database.ops/astroplant",
    );
    Pool::new(manager).expect("PostgreSQL connection pool could not be created.")
}

fn main() {
    env_logger::init();

    init_token_signer();

    let pg_pool = pg_pool();
    let rate_limit = rate_limit::leaky_bucket();

    let pg = helpers::pg(pg_pool);

    let all = rate_limit
        .and(
            path!("version")
                .map(|| ResponseBuilder::ok().body(VERSION))
                .or(path!("time")
                    .map(|| ResponseBuilder::ok().body(chrono::Utc::now().to_rfc3339())))
                .unify()
                .or(path!("kits").and(controllers::kit::router(pg.clone().boxed())))
                .unify()
                .or(path!("users").and(controllers::user::router(pg.clone().boxed())))
                .unify()
                .or(path!("me").and(controllers::me::router(pg.clone().boxed())))
                .unify(),
        )
        .and(warp::header("Accept"))
        .map(|response: Response, _accept: String| {
            // TODO: utilize Accept header, e.g. returning XML when requested.

            let mut http_response_builder = warp::http::response::Builder::new();
            http_response_builder.status(response.status_code());
            http_response_builder.header("Content-Type", "application/json");

            for (header, value) in response.headers() {
                http_response_builder.header(header.as_bytes(), value.clone());
            }

            match response.value() {
                Some(value) => http_response_builder
                    .body(serde_json::to_string(value).unwrap())
                    .unwrap(),
                None => http_response_builder.body("".to_owned()).unwrap(),
            }
        })
        .recover(handle_rejection)
        .with(warp::log("astroplant_rs_api::api"))
        // TODO: this wrapper might be better placed per-endpoint, to have accurate allowed metods
        .with(warp::cors().allow_any_origin().allow_methods(vec![
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ]).allow_headers(vec!["Content-Type"]));

    warp::serve(all).run(([127, 0, 0, 1], 8080));
}

/// Convert rejections into replies.
fn handle_rejection(rejection: Rejection) -> Result<impl Reply, Rejection> {
    use problem::{DescriptiveProblem, Problem};

    let reply = if let Some(problem) = rejection.find_cause::<Problem>() {
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
