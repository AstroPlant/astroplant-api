use futures::future::TryFutureExt;
use serde::{Deserialize, Serialize};
use warp::{filters::BoxedFilter, path, Filter, Rejection};

use crate::response::{Response, ResponseBuilder};
use crate::PgPooled;
use crate::{authentication, helpers, models, problem, views};

pub fn router(pg: BoxedFilter<(crate::PgPooled,)>) -> BoxedFilter<(Response,)> {
    //impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    trace!("Setting up kits router.");

    (warp::get2().and(kit_by_serial(pg.clone().boxed())))
        .or(warp::post2().and(reset_password(pg.clone().boxed())))
        .unify()
        .or(warp::path::end()
            .and(warp::get2())
            .and(kits(pg.clone().boxed())))
        .unify()
        .or(warp::path::end()
            .and(warp::post2())
            .and(create_kit(pg.boxed())))
        .unify()
        .boxed()
}

#[derive(Deserialize)]
struct CursorPage {
    after: Option<i32>,
}

/// Handles the `GET /kits/?after=afterId` route.
pub fn kits(
    pg: BoxedFilter<(crate::PgPooled,)>,
) -> impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    warp::query::query::<CursorPage>()
        .and(pg)
        .and_then(|cursor: CursorPage, conn: PgPooled| {
            helpers::threadpool_diesel_ok(move || {
                models::Kit::cursor_page(&conn, cursor.after, 100).map(|kits| {
                    kits.into_iter()
                        .map(|kit| views::Kit::from(kit))
                        .collect::<Vec<_>>()
                })
            })
        })
        .map(move |kits: Vec<views::Kit>| {
            let next_page_uri = kits.last().map(|last| format!("/kits?after={}", last.id));
            let mut response_builder = ResponseBuilder::ok();
            if let Some(next_page_uri) = next_page_uri {
                response_builder = response_builder.next_page_uri(next_page_uri);
            }
            response_builder.body(kits)
        })
}

/// Handles the `GET /kits/{kitSerial}` route.
pub fn kit_by_serial(
    pg: BoxedFilter<(crate::PgPooled,)>,
) -> impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    path!(String)
        .and(warp::path::end())
        .and(authentication::option_by_token())
        .and(pg.clone())
        .and_then(
            |kit_serial: String, user_id: Option<models::UserId>, conn: PgPooled| {
                helpers::fut_permission_or_forbidden(
                    conn,
                    user_id,
                    kit_serial,
                    crate::authorization::KitAction::View,
                )
                .map_ok(|(_, _, kit)| kit)
            },
        )
        .map(move |kit| ResponseBuilder::ok().body(views::Kit::from(kit)))
}

/// Handles the `POST /kits/{kitSerial}/password` route.
pub fn reset_password(
    pg: BoxedFilter<(crate::PgPooled,)>,
) -> impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    path!(String / "password")
        .and(warp::path::end())
        .and(authentication::option_by_token())
        .and(pg.clone())
        .and_then(
            |kit_serial: String, user_id: Option<models::UserId>, conn: PgPooled| {
                helpers::fut_permission_or_forbidden(
                    conn,
                    user_id,
                    kit_serial,
                    crate::authorization::KitAction::ResetPassword,
                )
                .map_ok(|(_, _, kit)| kit)
            },
        )
        .and(pg)
        .and_then(move |kit: models::Kit, conn: PgPooled| {
            async {
                helpers::threadpool_diesel_ok(move || {
                    let (update_kit, password) =
                        models::UpdateKit::unchanged_for_id(kit.id).reset_password();
                    update_kit.update(&conn)?;
                    Ok(password)
                })
                .await
                .map(|password| ResponseBuilder::ok().body(password))
            }
        })
}

/// Handles the `POST /kits` route.
pub fn create_kit(
    pg: BoxedFilter<(crate::PgPooled,)>,
) -> impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    use bigdecimal::{BigDecimal, FromPrimitive};
    use diesel::Connection;
    use validator::Validate;

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

    authentication::by_token()
        .and(crate::helpers::deserialize())
        .and(pg)
        .and_then(|user_id: models::UserId, kit: Kit, conn: crate::PgPooled| {
            async move {
                let (new_kit, password) = models::NewKit::new_with_generated_password(
                    kit.name,
                    kit.description,
                    kit.latitude.and_then(|l| BigDecimal::from_f64(l)),
                    kit.longitude.and_then(|l| BigDecimal::from_f64(l)),
                    kit.privacy_public_dashboard,
                    kit.privacy_show_on_map,
                );

                if let Err(validation_errors) = new_kit.validate() {
                    let invalid_parameters = problem::InvalidParameters::from(validation_errors);
                    return Err(warp::reject::custom(problem::Problem::InvalidParameters {
                        invalid_parameters,
                    }));
                };

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
                .await
            }
        })
}
