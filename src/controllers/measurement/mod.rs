use futures::future::FutureExt;
use serde::{Deserialize, Serialize};
use warp::{filters::BoxedFilter, Filter, Rejection};

use crate::database::PgPool;
use crate::problem::AppResult;
use crate::response::{Response, ResponseBuilder};
use crate::{authentication, authorization, helpers, models, views};

pub fn router(pg: PgPool) -> BoxedFilter<(AppResult<Response>,)> {
    //impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    tracing::trace!("Setting up measurements router.");

    kit_aggregate_measurements(pg.clone()).boxed()
}

/// Handles the `GET /kits/{kitSerial}/aggregate-measurements` route.
fn kit_aggregate_measurements(
    pg: PgPool,
) -> impl Filter<Extract = (AppResult<Response>,), Error = Rejection> + Clone {
    #[derive(Clone, Debug, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Query {
        cursor: Option<String>,
        configuration: Option<i32>,
        peripheral: Option<i32>,
        quantity_type: Option<i32>,
    }

    async fn implementation(
        pg: PgPool,
        kit_serial: String,
        user_id: Option<models::UserId>,
        query: Query,
    ) -> AppResult<Response> {
        use crate::cursors;
        use std::convert::TryFrom;

        let mut out_query = query.clone();
        let cursor = (&query).cursor.as_ref().map(|s| s.parse()).transpose()?;
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
            models::AggregateMeasurement::page(
                &conn,
                kit.get_id(),
                query.configuration,
                query.peripheral,
                query.quantity_type,
                cursor,
            )
        })
        .await?;

        if let Some(next_cursor) =
            cursors::AggregateMeasurements::next_from_page(&aggregate_measurements)
        {
            out_query.cursor = Some(next_cursor.into());
            let next_page_uri = format!(
                "{}?{}",
                base_uri,
                serde_urlencoded::to_string(&out_query).unwrap()
            );
            response = response.link(&next_page_uri, "next");
        }

        let body = aggregate_measurements
            .into_iter()
            .map(|aggregate_measurement| {
                views::AggregateMeasurement::try_from(aggregate_measurement)
            })
            .collect::<Result<Vec<_>, _>>()?;

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
