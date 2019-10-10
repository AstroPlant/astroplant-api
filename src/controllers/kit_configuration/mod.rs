use serde::Deserialize;
use warp::{filters::BoxedFilter, Filter, Rejection};

use crate::response::{Response, ResponseBuilder};
use crate::PgPooled;
use crate::{authentication, helpers, models, views};

pub fn router(pg: BoxedFilter<(crate::PgPooled,)>) -> BoxedFilter<(Response,)> {
    //impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    trace!("Setting up configurations router.");

    (warp::get2().and(configurations_by_kit_serial(pg.clone().boxed())))
        .or(warp::post2().and(create_configuration(pg.clone().boxed())))
        .unify()
        .boxed()
}

/// Handles the `GET /kit-configurations/{kitSerial}` route.
pub fn configurations_by_kit_serial(
    pg: BoxedFilter<(crate::PgPooled,)>,
) -> impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    use diesel::Connection;
    use futures::future::Future;
    use itertools::Itertools;
    use std::collections::HashMap;

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct KitSerial {
        kit_serial: String,
    }

    warp::query::query::<KitSerial>()
        .map(|query: KitSerial| query.kit_serial)
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
                .map(|(_, _, kit)| kit)
            },
        )
        .and(pg)
        .and_then(|kit, conn: PgPooled| {
            helpers::threadpool_diesel_ok(move || {
                conn.transaction(|| {
                    let kit_configurations =
                        models::KitConfiguration::configurations_of_kit(&conn, &kit)?;
                    let kit_peripherals = models::Peripheral::peripherals_of_kit(&conn, &kit)?;
                    let mut kit_peripherals: HashMap<i32, Vec<views::Peripheral>> = kit_peripherals
                        .into_iter()
                        .map(|p| (p.kit_configuration_id, views::Peripheral::from(p)))
                        .into_group_map();
                    let kit_configurations_with_peripherals: Vec<
                        views::KitConfigurationWithPeripherals<views::Peripheral>,
                    > = kit_configurations
                        .into_iter()
                        .map(|c| views::KitConfiguration::from(c))
                        .map(|c| {
                            let id = c.id;
                            c.with_peripherals(
                                kit_peripherals.remove(&id).unwrap_or_else(|| vec![]),
                            )
                        })
                        .collect();
                    Ok(ResponseBuilder::ok().body(kit_configurations_with_peripherals))
                })
            })
        })
}

/// Handles the `POST /kit-configurations/{kitSerial}` route.
pub fn create_configuration(
    pg: BoxedFilter<(crate::PgPooled,)>,
) -> impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    use futures::future::Future;

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct KitSerial {
        kit_serial: String,
    }

    #[derive(Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    struct Configuration {
        description: Option<String>,
    }

    warp::query::query::<KitSerial>()
        .map(|query: KitSerial| query.kit_serial)
        .and(warp::path::end())
        .and(authentication::option_by_token())
        .and(pg.clone())
        .and_then(
            |kit_serial: String, user_id: Option<models::UserId>, conn: PgPooled| {
                helpers::fut_permission_or_forbidden(
                    conn,
                    user_id,
                    kit_serial,
                    crate::authorization::KitAction::EditConfiguration,
                )
                .map(|(_, _, kit)| kit)
            },
        )
        .and(crate::helpers::deserialize())
        .and(pg)
        .and_then(
            |kit: models::Kit, configuration: Configuration, conn: PgPooled| {
                helpers::threadpool_diesel_ok(move || {
                    let new_configuration =
                        models::NewKitConfiguration::new(kit.get_id(), configuration.description);
                    let new_configuration = new_configuration.create(&conn)?;

                    Ok(
                        ResponseBuilder::ok()
                            .body(views::KitConfiguration::from(new_configuration)),
                    )
                })
            },
        )
}
