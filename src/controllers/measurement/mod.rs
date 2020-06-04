use futures::future::FutureExt;
use serde::{Deserialize, Serialize};
use warp::{filters::BoxedFilter, Filter, Rejection};

use crate::database::PgPool;
use crate::problem::AppResult;
use crate::response::{Response, ResponseBuilder};
use crate::{authentication, authorization, helpers, models, views};

pub fn router(pg: PgPool) -> BoxedFilter<(AppResult<Response>,)> {
    //impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    trace!("Setting up measurements router.");

    kit_aggregate_measurements(pg.clone()).boxed()
}

/// Handles the `GET /kits/{kitSerial}/aggregate-measurements` route.
fn kit_aggregate_measurements(
    pg: PgPool,
) -> impl Filter<Extract = (AppResult<Response>,), Error = Rejection> + Clone {
    #[derive(Clone, Serialize, Deserialize)]
    struct Query {
        cursor: Option<String>,
    }

    async fn implementation(
        pg: PgPool,
        kit_serial: String,
        user_id: Option<models::UserId>,
        query: Query,
    ) -> AppResult<Response> {
        use crate::cursors;

        let cursor = query.cursor.map(|s| s.parse()).transpose()?;
        let base_uri = format!("/kits/{}/aggregate-measurements", kit_serial);

        let (_user, _membership, kit) = helpers::fut_kit_permission_or_forbidden(
            pg.clone(),
            user_id,
            kit_serial,
            authorization::KitAction::View,
        )
        .await?;

        let conn = pg.get().await?;
        let mut response = ResponseBuilder::ok();
        let aggregate_measurements = helpers::threadpool(move || {
            models::AggregateMeasurement::page(&conn, kit.get_id(), cursor)
        })
        .await?;

        if let Some(next_cursor) =
            cursors::AggregateMeasurements::next_from_page(&aggregate_measurements)
        {
            let next_page_uri = format!(
                "{}?{}",
                base_uri,
                serde_urlencoded::to_string(Query {
                    cursor: Some(next_cursor.into())
                })
                .unwrap()
            );
            response = response.link(&next_page_uri, "next");
        }

        let body: Vec<_> = aggregate_measurements
            .into_iter()
            .map(|aggregate_measurement| views::AggregateMeasurement::from(aggregate_measurement))
            .collect();

        Ok(response.body(body))
    }

    warp::get()
        .and(warp::path!("kits" / String / "aggregate-measurements"))
        .and(authentication::option_by_token())
        .and(warp::query())
        .and_then(move |kit_serial, user_id, query: Query| {
            implementation(pg.clone(), kit_serial, user_id, query).never_error()
        })
}
