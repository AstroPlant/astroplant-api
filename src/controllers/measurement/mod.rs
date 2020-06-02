use futures::future::FutureExt;
use warp::{filters::BoxedFilter, Filter, Rejection};

use crate::database::PgPool;
use crate::problem::{AppResult, Problem};
use crate::response::{Response, ResponseBuilder};
use crate::{helpers, models, views};

pub fn router(pg: PgPool) -> BoxedFilter<(AppResult<Response>,)> {
    //impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    trace!("Setting up measurements router.");

    kit_aggregate_measurements(pg.clone()).boxed()
}

/// Handles the `GET /kits/{kitSerial}/aggregate-measurements` route.
fn kit_aggregate_measurements(
    pg: PgPool,
) -> impl Filter<Extract = (AppResult<Response>,), Error = Rejection> + Clone {
    async fn implementation(pg: PgPool, kit: models::Kit) -> AppResult<Response> {
        let conn = pg.get().await?;
        let aggregate_measurements = helpers::threadpool(move || {
            Ok::<Vec<_>, Problem>(
                models::AggregateMeasurement::recent_measurements(&conn, kit.get_id())?
                    .into_iter()
                    .map(|aggregate_measurement| {
                        views::AggregateMeasurement::from(aggregate_measurement)
                    })
                    .collect(),
            )
        })
        .await?;
        Ok(ResponseBuilder::ok().body(aggregate_measurements))
    }

    warp::get()
        .and(
            helpers::authorization_user_kit_from_filter(
                warp::path!("kits" / String / "aggregate-measurements").boxed(),
                pg.clone(),
                crate::authorization::KitAction::View,
            )
            .map(|_, _, kit| kit),
        )
        .and_then(move |kit: models::Kit| implementation(pg.clone(), kit).never_error())
}
