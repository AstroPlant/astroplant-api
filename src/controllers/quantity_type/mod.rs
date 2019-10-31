use futures::future::TryFutureExt;
use serde::Deserialize;
use warp::{filters::BoxedFilter, Filter, Rejection};

use crate::response::{Response, ResponseBuilder};
use crate::PgPooled;
use crate::{helpers, models, views};

pub fn router(pg: BoxedFilter<(crate::PgPooled,)>) -> BoxedFilter<(Response,)> {
    //impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    trace!("Setting up quantity types router.");

    warp::path::end()
        .and(warp::get2())
        .and(quantity_types(pg.clone()))
        .boxed()
}

/// Handles the `GET /quantity-types/?after=afterId` route.
pub fn quantity_types(
    pg: BoxedFilter<(crate::PgPooled,)>,
) -> impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    #[derive(Deserialize)]
    struct CursorPage {
        after: Option<i32>,
    }

    warp::query::query::<CursorPage>()
        .and(pg)
        .and_then(|cursor: CursorPage, conn: PgPooled| {
            helpers::threadpool_diesel_ok(move || {
                models::QuantityType::cursor_page(&conn, cursor.after, 100).map(
                    |quantity_types| {
                        quantity_types
                            .into_iter()
                            .map(|quantity_type| views::QuantityType::from(quantity_type))
                            .collect::<Vec<_>>()
                    },
                )
            })
            .map_ok(move |quantity_types| {
                let next_page_uri = quantity_types
                    .last()
                    .map(|last| format!("/quantity-types?after={}", last.id));
                let mut response_builder = ResponseBuilder::ok();
                if let Some(next_page_uri) = next_page_uri {
                    response_builder = response_builder.next_page_uri(next_page_uri);
                }
                response_builder.body(quantity_types)
            })
        })
}
