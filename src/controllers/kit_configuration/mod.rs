mod peripheral;

use serde::Deserialize;
use warp::{filters::BoxedFilter, path, Filter, Rejection};

use crate::utils::deserialize_some;
use crate::response::{Response, ResponseBuilder};
use crate::PgPooled;
use crate::{helpers, models, problem, views};

pub fn router(pg: BoxedFilter<(crate::PgPooled,)>) -> BoxedFilter<(Response,)> {
    //impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    trace!("Setting up configurations router.");

    configurations_by_kit_serial(pg.clone())
        .or(create_configuration(pg.clone()))
        .unify()
        .or(patch_configuration(pg.clone()))
        .unify()
        .or(peripheral::router(pg.clone()))
        .unify()
        .boxed()
}

/// Get the kit configuration from the path or error with 404.
fn get_kit_configuration(
    pg: BoxedFilter<(crate::PgPooled,)>,
) -> BoxedFilter<(models::KitConfiguration,)> {
    use futures::future::Future;

    path!(i32)
        .and(pg)
        .and_then(|configuration_id: i32, conn: crate::PgPooled| {
            helpers::threadpool_diesel_ok(move || {
                let configuration = match models::KitConfiguration::by_id(
                    &conn,
                    models::KitConfigurationId(configuration_id),
                )? {
                    Some(configuration) => configuration,
                    None => return Ok(Err(warp::reject::custom(problem::NOT_FOUND))),
                };

                Ok(Ok(configuration))
            })
            .then(helpers::flatten_result)
        })
        .boxed()
}

/// Authorize the user against the kit in the query for the given action, get the kit configuration
/// from the path, and compare the configuration's kit id and with the kit's id. Rejects the request
/// on error.
fn authorize_and_get_kit_configuration(
    pg: BoxedFilter<(crate::PgPooled,)>,
    action: crate::authorization::KitAction,
) -> BoxedFilter<(
    Option<models::User>,
    Option<models::KitMembership>,
    models::Kit,
    models::KitConfiguration,
)> {
    get_kit_configuration(pg.clone())
        .and(helpers::authorization_user_kit_from_query(
            pg.clone(),
            action,
        ))
        .and_then(
            |kit_configuration: models::KitConfiguration,
             user: Option<models::User>,
             kit_membership: Option<models::KitMembership>,
             kit: models::Kit| {
                if kit_configuration.kit_id == kit.id {
                    Ok((user, kit_membership, kit, kit_configuration))
                } else {
                    Err(warp::reject::custom(problem::NOT_FOUND))
                }
            },
        )
        .untuple_one()
        .boxed()
}

/// Handles the `GET /kit-configurations?kitSerial={kitSerial}` route.
fn configurations_by_kit_serial(
    pg: BoxedFilter<(crate::PgPooled,)>,
) -> impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    use diesel::Connection;
    use itertools::Itertools;
    use std::collections::HashMap;

    warp::get2()
        .and(warp::path::end())
        .and(
            helpers::authorization_user_kit_from_query(
                pg.clone(),
                crate::authorization::KitAction::View,
            )
            .map(|_, _, kit| kit),
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

/// Handles the `POST /kit-configurations?kitSerial={kitSerial}` route.
fn create_configuration(
    pg: BoxedFilter<(crate::PgPooled,)>,
) -> impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    #[derive(Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    struct Configuration {
        description: Option<String>,
    }

    warp::post2()
        .and(warp::path::end())
        .and(
            helpers::authorization_user_kit_from_query(
                pg.clone(),
                crate::authorization::KitAction::EditConfiguration,
            )
            .map(|_, _, kit| kit),
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

/// Handles the `PATCH /kit-configurations/{kitConfigurationId}?kitSerial={kitSerial}` route.
///
/// If the configuration is set active, all other configurations of the kit are deactivated.
fn patch_configuration(
    pg: BoxedFilter<(crate::PgPooled,)>,
) -> impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    use diesel::Connection;

    #[derive(Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    struct KitConfigurationPatch {
        #[serde(default, deserialize_with = "deserialize_some")]
        description: Option<Option<String>>,
        active: Option<bool>,
    }

    warp::patch()
        .and(authorize_and_get_kit_configuration(
            pg.clone(),
            crate::authorization::KitAction::EditConfiguration,
        ))
        .and(warp::path::end())
        .and(crate::helpers::deserialize())
        .and(pg)
        .and_then(
            |_user,
             _kit_membership,
             kit: models::Kit,
             configuration: models::KitConfiguration,
             configuration_patch: KitConfigurationPatch,
             conn: PgPooled| {
                let patch = models::UpdateKitConfiguration {
                    id: configuration.id,
                    description: configuration_patch.description,
                    active: configuration_patch.active,
                    never_used: match configuration_patch.active {
                        Some(true) => Some(false),
                        _ => None,
                    },
                };

                helpers::threadpool_diesel_ok(move || {
                    conn.transaction(|| {
                        if let Some(active) = patch.active {
                            if active != configuration.active {
                                models::KitConfiguration::deactivate_all_of_kit(&conn, &kit)?;
                            }
                        }
                        let patched_configuration = patch.update(&conn)?;

                        Ok(ResponseBuilder::ok()
                            .body(views::KitConfiguration::from(patched_configuration)))
                    })
                })
            },
        )
}
