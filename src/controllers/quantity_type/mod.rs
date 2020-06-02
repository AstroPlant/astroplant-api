use futures::future::FutureExt;
use serde::Deserialize;
use warp::{filters::BoxedFilter, Filter, Rejection};

use crate::database::PgPool;
use crate::problem::{AppResult, Problem};
use crate::response::{Response, ResponseBuilder};
use crate::{helpers, models, views};

pub fn router(pg: PgPool) -> BoxedFilter<(AppResult<Response>,)> {
    trace!("Setting up quantity types router.");

    warp::path::end()
        .and(warp::get())
        .and(quantity_types(pg.clone()))
        .boxed()
}

/// Handles the `GET /quantity-types/?after=afterId` route.
pub fn quantity_types(
    pg: PgPool,
) -> impl Filter<Extract = (AppResult<Response>,), Error = Rejection> + Clone {
    #[derive(Deserialize)]
    struct CursorPage {
        after: Option<i32>,
    }

    async fn implementation(pg: PgPool, cursor: CursorPage) -> AppResult<Response> {
        let conn = pg.get().await?;
        let quantity_types = helpers::threadpool(move || {
            Ok::<_, Problem>(
                models::QuantityType::cursor_page(&conn, cursor.after, 100)?
                    .into_iter()
                    .map(|quantity_type| views::QuantityType::from(quantity_type))
                    .collect::<Vec<_>>(),
            )
        })
        .await?;
        let next_page_uri = quantity_types
            .last()
            .map(|last| format!("/quantity-types?after={}", last.id));
        let mut response_builder = ResponseBuilder::ok();
        if let Some(next_page_uri) = next_page_uri {
            response_builder = response_builder.next_page_uri(next_page_uri);
        }
        Ok(response_builder.body(quantity_types))
    }

    warp::query::query::<CursorPage>()
        .and_then(move |cursor: CursorPage| implementation(pg.clone(), cursor).never_error())
}
