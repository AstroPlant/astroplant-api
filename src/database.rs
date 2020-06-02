use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use futures::future::TryFutureExt;
use warp::{Filter, Rejection};

use crate::helpers;
use crate::problem::{AppResult, INTERNAL_SERVER_ERROR};

// type PgPool = Pool<ConnectionManager<PgConnection>>;
pub type PgPooled = PooledConnection<ConnectionManager<PgConnection>>;

#[derive(Clone)]
pub struct PgPool(Pool<ConnectionManager<PgConnection>>);

impl PgPool {
    pub fn new(url: String, connection_timeout: std::time::Duration) -> Self {
        let manager = ConnectionManager::<PgConnection>::new(url);
        let pool = Pool::builder()
            .connection_timeout(connection_timeout)
            .build(manager)
            .expect("PostgreSQL connection pool could not be created.");
        Self(pool)
    }

    pub async fn get(self) -> AppResult<PgPooled> {
        // TODO: check whether PgPool::get actually needs to be run in a threadpool
        helpers::threadpool(move || self.0.get().map_err(|_| INTERNAL_SERVER_ERROR)).await
    }

    /// Create a filter to get a PostgreSQL connection from a PostgreSQL connection pool.
    pub fn filter(self) -> impl Filter<Extract = (PgPooled,), Error = Rejection> + Clone {
        warp::any().and_then(move || self.clone().get().err_into::<Rejection>())
    }
}
