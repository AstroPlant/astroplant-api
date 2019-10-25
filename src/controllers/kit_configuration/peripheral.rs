use futures::future::FutureExt;
use serde::Deserialize;
use validator::Validate;
use warp::{filters::BoxedFilter, path, Filter, Rejection};

use crate::response::{Response, ResponseBuilder};
use crate::PgPooled;
use crate::{helpers, models, problem, views};

pub fn router(pg: BoxedFilter<(crate::PgPooled,)>) -> BoxedFilter<(Response,)> {
    //impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    trace!("Setting up configurations/peripheral router.");

    add_peripheral_to_configuration(pg.clone())
        .or(patch_or_delete_peripheral(pg.clone()))
        .unify()
        .boxed()
}

fn peripheral_base_filter(
    pg: BoxedFilter<(crate::PgPooled,)>,
    action: crate::authorization::KitAction,
) -> BoxedFilter<(
    Option<models::User>,
    Option<models::KitMembership>,
    models::Kit,
    models::KitConfiguration,
)> {
    super::authorize_and_get_kit_configuration(pg, action)
        .and(path!("peripherals"))
        .boxed()
}

fn check_configuration(
    configuration: &serde_json::Value,
    peripheral_definition: &models::PeripheralDefinition,
) -> Result<(), problem::Problem> {
    let mut scope = valico::json_schema::Scope::new();
    let schema =
        match scope.compile_and_return(peripheral_definition.configuration_schema.clone(), false) {
            Ok(schema) => schema,
            Err(_) => {
                error!(
                    "peripheral definition with id {} has an invalid configuration schema",
                    peripheral_definition.id
                );
                return Err(problem::INTERNAL_SERVER_ERROR);
            }
        };

    let mut invalid_parameters = problem::InvalidParameters::new();
    if !schema.validate(configuration).is_strictly_valid() {
        invalid_parameters.add("configuration", problem::InvalidParameterReason::Other)
    }

    if !invalid_parameters.is_empty() {
        return Err(invalid_parameters.into_problem());
    }

    Ok(())
}

/// Handles the `POST /kit-configurations/{kitConfigurationId}/peripherals?kitSerial={kitSerial}`
/// route.
fn add_peripheral_to_configuration(
    pg: BoxedFilter<(crate::PgPooled,)>,
) -> impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    use diesel::prelude::*;
    use diesel::Connection;

    #[derive(Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    struct Peripheral {
        peripheral_definition_id: i32,
        name: String,
        configuration: serde_json::Value,
    }

    peripheral_base_filter(
        pg.clone(),
        crate::authorization::KitAction::EditConfiguration,
    )
    .and(warp::post2())
    .and(warp::path::end())
    .and(crate::helpers::deserialize())
    .and(pg)
    .and_then(
        |_user,
         _kit_membership,
         kit: models::Kit,
         configuration: models::KitConfiguration,
         peripheral: Peripheral,
         conn: PgPooled| {
            futures::future::ready(helpers::guard(
                (kit, configuration, peripheral, conn),
                |(_, configuration, _, _)| {
                    if configuration.never_used {
                        None
                    } else {
                        Some(warp::reject::custom(
                            problem::InvalidParameterReason::AlreadyActivated
                                .singleton("configurationId")
                                .into_problem(),
                        ))
                    }
                },
            ))
        },
    )
    .untuple_one()
    .and_then(
        |kit: models::Kit,
         configuration: models::KitConfiguration,
         peripheral: Peripheral,
         conn: PgPooled| {
            helpers::threadpool_diesel_ok(move || {
                conn.transaction(|| {
                    let new_peripheral = models::NewPeripheral::new(
                        kit.get_id(),
                        configuration.get_id(),
                        models::PeripheralDefinitionId(peripheral.peripheral_definition_id),
                        peripheral.name,
                        peripheral.configuration,
                    );

                    if let Err(validation_errors) = new_peripheral.validate() {
                        let invalid_parameters =
                            problem::InvalidParameters::from(validation_errors);
                        return Ok(Err(warp::reject::custom(invalid_parameters.into_problem())));
                    }

                    let definition = match models::PeripheralDefinition::by_id(
                        &conn,
                        peripheral.peripheral_definition_id,
                    )
                    .optional()?
                    {
                        Some(definition) => definition,
                        None => {
                            return Ok(Err(warp::reject::custom(
                                problem::InvalidParameterReason::NotFound
                                    .singleton("peripheralDefinitionId")
                                    .into_problem(),
                            )))
                        }
                    };

                    if let Err(problem) =
                        check_configuration(&new_peripheral.configuration, &definition)
                    {
                        return Ok(Err(warp::reject::custom(problem)));
                    }

                    let created_peripheral = new_peripheral.create(&conn)?;

                    Ok(Ok(
                        ResponseBuilder::ok().body(views::Peripheral::from(created_peripheral))
                    ))
                })
            })
            .map(helpers::flatten_result)
        },
    )
}

/// Handles the `PATCH` and `DELETE /kit-configurations/{kitConfigurationId}/peripherals/{peripheralId}?kitSerial={kitSerial}`
/// route.
fn patch_or_delete_peripheral(
    pg: BoxedFilter<(crate::PgPooled,)>,
) -> impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    use diesel::prelude::*;
    use diesel::Connection;

    #[derive(Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    struct PeripheralPatch {
        name: Option<String>,
        configuration: Option<serde_json::Value>,
    }

    let base = peripheral_base_filter(
        pg.clone(),
        crate::authorization::KitAction::EditConfiguration,
    )
    .and(
        path!(i32)
            .map(models::PeripheralId)
            .and(pg.clone())
            .and_then(|peripheral_id: models::PeripheralId, conn: PgPooled| {
                helpers::threadpool_diesel_ok(move || {
                    match models::Peripheral::by_id(&conn, peripheral_id)? {
                        Some(peripheral) => Ok(Ok(peripheral)),
                        None => Ok(Err(warp::reject::custom(problem::NOT_FOUND))),
                    }
                })
                .map(helpers::flatten_result)
            }),
    )
    .and(warp::path::end())
    .and(pg)
    .and_then(
        |_user,
         _kit_membership,
         _kit: models::Kit,
         configuration: models::KitConfiguration,
         peripheral: models::Peripheral,
         conn: PgPooled| {
            async {
                helpers::guard(
                    (configuration, peripheral, conn),
                    |(configuration, _, _)| {
                        if configuration.never_used {
                            None
                        } else {
                            Some(warp::reject::custom(
                                problem::InvalidParameterReason::AlreadyActivated
                                    .singleton("configurationId")
                                    .into_problem(),
                            ))
                        }
                    },
                )
            }
        },
    )
    .untuple_one();

    (base
        .clone()
        .and(warp::patch())
        .and(crate::helpers::deserialize())
        .and_then(
            |_configuration: models::KitConfiguration,
             peripheral: models::Peripheral,
             conn: PgPooled,
             peripheral_change: PeripheralPatch| {
                helpers::threadpool_diesel_ok(move || {
                    conn.transaction(|| {
                        let patched_peripheral = models::UpdatePeripheral {
                            id: peripheral.id,
                            name: peripheral_change.name,
                            configuration: peripheral_change.configuration,
                        };

                        if let Err(validation_errors) = patched_peripheral.validate() {
                            let invalid_parameters =
                                problem::InvalidParameters::from(validation_errors);
                            return Ok(Err(warp::reject::custom(
                                invalid_parameters.into_problem(),
                            )));
                        }

                        let definition = match models::PeripheralDefinition::by_id(
                            &conn,
                            peripheral.peripheral_definition_id,
                        )
                        .optional()?
                        {
                            Some(definition) => definition,
                            None => {
                                return Ok(Err(warp::reject::custom(
                                    problem::INTERNAL_SERVER_ERROR,
                                )))
                            }
                        };

                        if let Some(configuration) = patched_peripheral.configuration.as_ref() {
                            if let Err(problem) = check_configuration(configuration, &definition) {
                                return Ok(Err(warp::reject::custom(problem)));
                            }
                        }

                        let updated_peripheral = patched_peripheral.update(&conn)?;

                        Ok(Ok(
                            ResponseBuilder::ok().body(views::Peripheral::from(updated_peripheral))
                        ))
                    })
                })
                .map(helpers::flatten_result)
            },
        ))
    .or(base.and(warp::delete2()).and_then(
        |_configuration: models::KitConfiguration,
         peripheral: models::Peripheral,
         conn: PgPooled| {
            helpers::threadpool_diesel_ok(move || {
                peripheral.delete(&conn)?;
                Ok(ResponseBuilder::ok().empty())
            })
        },
    ))
    .unify()
}
