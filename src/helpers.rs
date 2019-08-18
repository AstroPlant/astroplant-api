use crate::problem::{Problem, INTERNAL_SERVER_ERROR};

use futures::future::{poll_fn, Future};
use serde::de::DeserializeOwned;
use warp::{Filter, Rejection};

pub fn fut_threadpool<F, T>(f: F) -> impl Future<Item = T, Error = tokio_threadpool::BlockingError>
where
    F: FnOnce() -> T,
{
    let mut f_only_once = Some(f);
    poll_fn(move || {
        tokio_threadpool::blocking(|| {
            let f = f_only_once.take().unwrap();
            f()
        })
    })
}

pub fn threadpool<F, T>(f: F) -> impl Future<Item = T, Error = Rejection>
where
    F: FnOnce() -> T,
{
    fut_threadpool(f).map_err(|_| warp::reject::custom(INTERNAL_SERVER_ERROR))
}

pub fn json_decode<T>() -> impl Filter<Extract = (T,), Error = Rejection> + Copy
where
    T: DeserializeOwned + Send,
{
    warp::body::json().or_else(|_| Err(warp::reject::custom(Problem::InvalidJson)))
}

pub fn pg(
    pg_pool: crate::PgPool,
) -> impl Filter<Extract = (crate::PgPooled,), Error = Rejection> + Clone {
    warp::any()
        .map(move || pg_pool.clone())
        .and_then(|pg_pool: crate::PgPool| match pg_pool.get() {
            Ok(pg_pooled) => Ok(pg_pooled),
            Err(_) => Err(warp::reject::custom(INTERNAL_SERVER_ERROR)),
        })
}

pub fn ok_or_internal_error<T, E>(r: Result<T, E>) -> Result<T, Rejection> {
    match r {
        Ok(value) => Ok(value),
        Err(_) => Err(warp::reject::custom(INTERNAL_SERVER_ERROR)),
    }
}
