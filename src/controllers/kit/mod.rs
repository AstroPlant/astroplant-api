use crate::problem::{INTERNAL_SERVER_ERROR, NOT_FOUND};

use serde::Deserialize;
use warp::{filters::BoxedFilter, path, Filter, Rejection, Reply};

pub fn router(
    pg: BoxedFilter<(crate::PgPooled,)>,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    trace!("Setting up kits router.");

    kit_by_id(pg.clone().boxed()).map(|reply| reply)
        .or(warp::path::end().and(kits(pg.boxed())))
}

#[derive(Deserialize)]
struct CursorPage {
    after: Option<i32>,
}

/// Handles the `GET /kits/?after=afterId` route.
pub fn kits(
    pg: BoxedFilter<(crate::PgPooled,)>,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    use crate::{helpers, models};
    use crate::{serialize, PgPooled};

    use futures::future::Future;

    warp::query::query::<CursorPage>()
        .and(warp::header::header("host"))
        .and(pg)
        .and_then(|cursor: CursorPage, host: String, conn: PgPooled| {
            helpers::fut_threadpool(move || {
                models::Kit::cursor_page(&conn, cursor.after, 100)
                    .map(|kits| {
                        kits.into_iter()
                            .map(|kit| kit.encodable())
                            .collect::<Vec<_>>()
                    })
                    .map_err(|_| warp::reject::custom(INTERNAL_SERVER_ERROR))
            })
            .map_err(|_| warp::reject::custom(INTERNAL_SERVER_ERROR))
            .then(|v| match v {
                Ok(t) => t,
                Err(r) => Err(r),
            })
            .map(move |kits| {
                let reply = serialize(&kits);
                if let Some(last) = kits.last() {
                    let reply = warp::reply::with_header(
                        reply,
                        "x-next",
                        format!("http://{}/kits?after={}", host, last.id),
                    );
                    reply.into_response()
                } else {
                    reply.into_response()
                }
            })
        })
}

/// Handles the `GET /kits/{kitId}` route.
pub fn kit_by_id(
    pg: BoxedFilter<(crate::PgPooled,)>,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    use crate::{helpers, models};
    use crate::{serialize, PgPooled};

    use futures::future::Future;

    path!(i32).and(pg).and_then(|id: i32, conn: PgPooled| {
        helpers::fut_threadpool(move || {
            models::Kit::by_id(&conn, id).map_err(|_| warp::reject::custom(NOT_FOUND))
        })
        .map_err(|_| warp::reject::custom(INTERNAL_SERVER_ERROR))
        .then(|v| match v {
            Ok(t) => t,
            Err(r) => Err(r),
        })
        .map(move |kit| {
            let reply = serialize(&kit.encodable());
            reply.into_response()
        })
    })
}
