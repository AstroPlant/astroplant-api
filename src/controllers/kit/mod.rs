use crate::problem::{INTERNAL_SERVER_ERROR, NOT_FOUND};

use serde::{Deserialize, Serialize};
use warp::{filters::BoxedFilter, path, Filter, Rejection};

use crate::authentication::authenticate_by_token;
use crate::response::{Response, ResponseBuilder};
use crate::views;

pub fn router(
    pg: BoxedFilter<(crate::PgPooled,)>,
) -> impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    trace!("Setting up kits router.");

    kit_by_id(pg.clone().boxed())
        .or(warp::path::end()
            .and(warp::get2())
            .and(kits(pg.clone().boxed())))
        .unify()
        .or(warp::path::end()
            .and(warp::post2())
            .and(create_kit(pg.boxed())))
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
    use crate::PgPooled;
    use crate::{helpers, models};

    use futures::future::Future;

    warp::query::query::<CursorPage>()
        .and(pg)
        .and_then(|cursor: CursorPage, conn: PgPooled| {
            helpers::fut_threadpool(move || {
                models::Kit::cursor_page(&conn, cursor.after, 100)
                    .map(|kits| {
                        kits.into_iter()
                            .map(|kit| views::Kit::from(kit))
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
                let mut response_builder = ResponseBuilder::ok();
                if let Some(next_page_uri) = next_page_uri {
                    response_builder = response_builder.next_page_uri(next_page_uri);
                }
                response_builder.body(kits)
            })
        })
}

/// Handles the `GET /kits/{kitId}` route.
pub fn kit_by_id(
    pg: BoxedFilter<(crate::PgPooled,)>,
) -> impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    use crate::PgPooled;
    use crate::{helpers, models};

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
        .map(move |kit| ResponseBuilder::ok().body(views::Kit::from(kit)))
    })
}

/// Handles the `POST /kits` route.
pub fn create_kit(
    pg: BoxedFilter<(crate::PgPooled,)>,
) -> impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    use crate::{helpers, models, problem};

    use bigdecimal::{BigDecimal, FromPrimitive};
    use diesel::Connection;
    use validator::Validate;
    use futures::future::{self, Future};

    #[derive(Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    struct Kit {
        name: Option<String>,
        description: Option<String>,
        latitude: Option<f64>,
        longitude: Option<f64>,
        privacy_public_dashboard: bool,
        privacy_show_on_map: bool,
    }

    #[derive(Serialize, Debug)]
    #[serde(rename_all = "camelCase")]
    struct Created {
        kit_serial: String,
        password: String,
    }

    authenticate_by_token()
        .and(crate::helpers::deserialize())
        .and(pg)
        .and_then(|user_id: models::UserId, kit: Kit, conn: crate::PgPooled| {
            let (new_kit, password) = models::NewKit::new_with_generated_password(
                kit.name,
                kit.description,
                kit.latitude.and_then(|l| BigDecimal::from_f64(l)),
                kit.longitude.and_then(|l| BigDecimal::from_f64(l)),
                kit.privacy_public_dashboard,
                kit.privacy_show_on_map,
            );

            future::result(match new_kit.validate() {
                // Does not strictly have to be wrapped in a future, but makes naming the return
                // type easier.
                Ok(_) => Ok(()),
                Err(validation_errors) => {
                    let invalid_parameters = problem::InvalidParameters::from(validation_errors);
                    Err(warp::reject::custom(problem::Problem::InvalidParameters {
                        invalid_parameters,
                    }))
                }
            })
            .and_then(move |_| {
                helpers::threadpool_diesel_ok(move || {
                    conn.transaction(|| {
                        let created_kit: models::Kit = new_kit.create(&conn)?;
                        let kit_serial = created_kit.serial;
                        debug!("Created kit \"{}\"", kit_serial);
                        let kit_id = models::KitId(created_kit.id);

                        models::NewKitMembership::new(user_id, kit_id, true, true).create(&conn)?;

                        let response = ResponseBuilder::created().body(Created {
                            kit_serial,
                            password,
                        });

                        Ok(response)
                    })
                })
            })
        })
}
