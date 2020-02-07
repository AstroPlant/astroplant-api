use warp::{filters::BoxedFilter, Filter, Rejection};

use crate::response::{Response, ResponseBuilder};
use crate::PgPooled;
use crate::{helpers, models, views};

pub fn router(pg: BoxedFilter<(crate::PgPooled,)>) -> BoxedFilter<(Response,)> {
    //impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    trace!("Setting up measurements router.");

    aggregate_measurements(pg.clone()).boxed()
}

/// Handles the `GET /measurements/aggregate-measurements?kitSerial={kitSerial}` route.
fn aggregate_measurements(
    pg: BoxedFilter<(crate::PgPooled,)>,
) -> impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    warp::get()
        .and(warp::path!("aggregate-measurements"))
        .and(
            helpers::authorization_user_kit_from_query(
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
