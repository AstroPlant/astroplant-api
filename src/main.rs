#[macro_use]
extern crate diesel;

use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use futures::future::Future;
use serde::Deserialize;
use warp::{self, path, Filter, Rejection, Reply};

type PgPool = Pool<ConnectionManager<PgConnection>>;
type PgPooled = PooledConnection<ConnectionManager<PgConnection>>;

mod error;
mod helpers;
mod schema;
use error::Error;
mod rate_limit;

mod controllers;
mod models;
mod views;

static VERSION: &str = "1.0.0-alpha";

fn pg_pool() -> PgPool {
    let manager = ConnectionManager::<PgConnection>::new(
        "postgres://astroplant:astroplant@database.ops/astroplant",
    );
    Pool::new(manager).expect("PostgreSQL connection pool could not be created.")
}

fn main() {
    let pg_pool = pg_pool();
    let rate_limit = rate_limit::leaky_bucket();

    let pg = warp::any()
        .map(move || pg_pool.clone())
        .and_then(|pg_pool: PgPool| match pg_pool.get() {
            Ok(pg_pooled) => Ok(pg_pooled),
            Err(_) => Err(warp::reject::custom(Error::InternalServer)),
        });

    let version = || VERSION;
    let time = || chrono::Utc::now().to_rfc3339();
    let test = pg
        .clone()
        .and_then(|conn: PgPooled| {
            helpers::fut_threadpool(move || {
                models::NewUser::new("test", "asd", "asd@asd.asd").create(&conn)
            })
            .map_err(|_| warp::reject::custom(Error::InternalServer))
        })
        .map(|res| {
            println!("{:?}", res);
            "asd"
        });

    let all = rate_limit
        .and(path!("version").map(version).map(|v| serialize(&v)))
        .or(path!("test").and(test))
        .or(path!("time").map(time).map(|t| serialize(&t)))
        .or(path!("kits").and(controllers::kit::kit_by_id(pg.clone().boxed())))
        .or(path!("kits").and(warp::path::end()).and(controllers::kit::kits(pg.boxed())))
        .recover(handle_rejection);

    warp::serve(all).run(([127, 0, 0, 1], 8080));
}

fn serialize<T>(val: &T) -> impl Reply
where
    T: serde::Serialize,
{
    warp::reply::json(val)
}

fn handle_rejection(rejection: Rejection) -> Result<impl Reply, Rejection> {
    if let Some(err) = rejection.find_cause::<Error>() {
        Ok(warp::reply::with_status(
            serde_json::to_string(&err.to_flat_error()).unwrap(),
            err.to_status_code(),
        ))
    } else {
        let err = if rejection.is_not_found() {
            Error::UnknownEndpoint
        } else {
            Error::InternalServer
        };
        Ok(warp::reply::with_status(
            serde_json::to_string(&err.to_flat_error()).unwrap(),
            err.to_status_code(),
        ))
    }
}
