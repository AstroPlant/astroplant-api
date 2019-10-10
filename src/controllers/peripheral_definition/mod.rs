use crate::problem::INTERNAL_SERVER_ERROR;

use futures::future::Future;
use serde::Deserialize;
use warp::{filters::BoxedFilter, Filter, Rejection};

use crate::response::{Response, ResponseBuilder};
use crate::PgPooled;
use crate::{helpers, models, views};

pub fn router(pg: BoxedFilter<(crate::PgPooled,)>) -> BoxedFilter<(Response,)> {
    //impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    trace!("Setting up users router.");

    warp::path::end()
        .and(warp::get2())
        .and(peripheral_definitions(pg.clone()))
        .boxed()
}

/// Handles the `GET /peripheral-definitions/?after=afterId` route.
pub fn peripheral_definitions(
    pg: BoxedFilter<(crate::PgPooled,)>,
) -> impl Filter<Extract = (Response,), Error = Rejection> + Clone {

    #[derive(Deserialize)]
    struct CursorPage {
        after: Option<i32>,
    }

    warp::query::query::<CursorPage>()
        .and(pg)
        .and_then(|cursor: CursorPage, conn: PgPooled| {
            helpers::fut_threadpool(move || {
                models::PeripheralDefinition::cursor_page(&conn, cursor.after, 100)
                    .map(|definitions| {
                        definitions
                            .into_iter()
                            .map(|definition| views::PeripheralDefinition::from(definition))
                            .collect::<Vec<_>>()
                    })
                    .map_err(|_| warp::reject::custom(INTERNAL_SERVER_ERROR))
            })
            .map_err(|_| warp::reject::custom(INTERNAL_SERVER_ERROR))
            .then(|v| match v {
                Ok(t) => t,
                Err(r) => Err(r),
            })
            .map(move |definitions| {
                let next_page_uri = definitions
                    .last()
                    .map(|last| format!("/peripheral-definitions?after={}", last.id));
                let mut response_builder = ResponseBuilder::ok();
                if let Some(next_page_uri) = next_page_uri {
                    response_builder = response_builder.next_page_uri(next_page_uri);
                }
                response_builder.body(definitions)
            })
        })
}
