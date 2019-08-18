use crate::problem::{INTERNAL_SERVER_ERROR, NOT_FOUND};

use serde::Deserialize;
use warp::{filters::BoxedFilter, path, Filter, Rejection};

use crate::response::Response;

pub fn router(
    pg: BoxedFilter<(crate::PgPooled,)>,
) -> impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    trace!("Setting up kits router.");

    kit_by_id(pg.clone().boxed())
        .or(warp::path::end().and(kits(pg.boxed())))
        .unify()
}

#[derive(Deserialize)]
struct CursorPage {
    after: Option<i32>,
}

/// Handles the `GET /kits/?after=afterId` route.
pub fn kits(
    pg: BoxedFilter<(crate::PgPooled,)>,
) -> impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    use crate::{helpers, models};
    use crate::PgPooled;

    use futures::future::Future;

    warp::query::query::<CursorPage>()
        .and(pg)
        .and_then(|cursor: CursorPage, conn: PgPooled| {
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
                let next_page_uri = kits.last().map(|last| format!("/kits?after={}", last.id));
                let mut response = Response::ok(kits);
                if let Some(next_page_uri) = next_page_uri {
                    response.set_next_page_uri(next_page_uri);
                }
                response
            })
        })
}

/// Handles the `GET /kits/{kitId}` route.
pub fn kit_by_id(
    pg: BoxedFilter<(crate::PgPooled,)>,
) -> impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    use crate::{helpers, models};
    use crate::PgPooled;

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
            Response::ok(kit.encodable())
        })
    })
}
