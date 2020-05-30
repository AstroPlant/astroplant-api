use warp::{filters::BoxedFilter, Filter, Rejection};

use crate::response::{Response, ResponseBuilder};
use crate::PgPooled;
use crate::{helpers, models, views};

pub fn router(pg: BoxedFilter<(crate::PgPooled,)>) -> BoxedFilter<(Response,)> {
    //impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    trace!("Setting up measurements router.");

    kit_aggregate_measurements(pg.clone()).boxed()
}

/// Handles the `GET /kits/{kitSerial}/aggregate-measurements` route.
fn kit_aggregate_measurements(
    pg: BoxedFilter<(crate::PgPooled,)>,
) -> impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    warp::get()
        .and(
            helpers::authorization_user_kit_from_filter(
                warp::path!("kits" / String / "aggregate-measurements").boxed(),
                pg.clone(),
                crate::authorization::KitAction::View,
            )
            .map(|_, _, kit| kit),
        )
        .and(pg)
        .and_then(|kit: models::Kit, conn: PgPooled| {
            helpers::threadpool_diesel_ok(move || {
                let aggregate_measurements: Vec<_> =
                    models::AggregateMeasurement::recent_measurements(&conn, kit.get_id())?
                        .into_iter()
                        .map(|aggregate_measurement| {
                            views::AggregateMeasurement::from(aggregate_measurement)
                        })
                        .collect();
                Ok(ResponseBuilder::ok().body(aggregate_measurements))
            })
        })
}
