use futures::future::FutureExt;
use serde::Deserialize;
use validator::Validate;
use warp::{filters::BoxedFilter, path, Filter, Rejection};

use crate::database::{PgPool, PgPooled};
use crate::problem::{self, AppResult};
use crate::response::{Response, ResponseBuilder};
use crate::{helpers, models, views};

pub fn router(pg: PgPool) -> BoxedFilter<(AppResult<Response>,)> {
    //impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    trace!("Setting up configurations/peripheral router.");

    add_peripheral_to_configuration(pg.clone())
        .or(patch_or_delete_peripheral(pg.clone()))
        .unify()
        .boxed()
}

fn peripheral_base_filter(
    pg: PgPool,
    action: crate::authorization::KitAction,
) -> BoxedFilter<(
    AppResult<(
        Option<models::User>,
        Option<models::KitMembership>,
        models::Kit,
        models::KitConfiguration,
    )>,
)> {
    super::authorize_and_get_kit_configuration(pg, action)
        .and(path!("peripherals" / ..))
        .boxed()
}

fn check_configuration(
    configuration: &serde_json::Value,
    peripheral_definition: &models::PeripheralDefinition,
) -> AppResult<()> {
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
    pg: PgPool,
) -> impl Filter<Extract = (AppResult<Response>,), Error = Rejection> + Clone {
    use diesel::prelude::*;
    use diesel::Connection;

    #[derive(Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    struct Peripheral {
        peripheral_definition_id: i32,
        name: String,
        configuration: serde_json::Value,
    }

    async fn implementation(
        pg: PgPool,
        kit: models::Kit,
        configuration: models::KitConfiguration,
        peripheral: Peripheral,
    ) -> AppResult<Response> {
        if !configuration.never_used {
            return Err(problem::InvalidParameterReason::AlreadyActivated
                .singleton("configurationId")
                .into_problem());
        }

        let peripheral_definition_id = peripheral.peripheral_definition_id;
        let new_peripheral = models::NewPeripheral::new(
            kit.get_id(),
            configuration.get_id(),
            models::PeripheralDefinitionId(peripheral.peripheral_definition_id),
            peripheral.name,
            peripheral.configuration,
        );
        if let Err(validation_errors) = new_peripheral.validate() {
            let invalid_parameters = problem::InvalidParameters::from(validation_errors);
            return Err(invalid_parameters.into_problem());
        }

        let conn = pg.get().await?;
        let created_peripheral = helpers::threadpool(move || {
            conn.transaction(|| {
                let definition =
                    match models::PeripheralDefinition::by_id(&conn, peripheral_definition_id)
                        .optional()?
                    {
                        Some(definition) => definition,
                        None => {
                            return Err(problem::InvalidParameterReason::NotFound
                                .singleton("peripheralDefinitionId")
                                .into_problem())
                        }
                    };

                check_configuration(&new_peripheral.configuration, &definition)?;

                Ok(new_peripheral.create(&conn)?)
            })
        })
        .await?;
        Ok(ResponseBuilder::ok().body(views::Peripheral::from(created_peripheral)))
    }

    peripheral_base_filter(
        pg.clone(),
        crate::authorization::KitAction::EditConfiguration,
    )
    .and(warp::post())
    .and(warp::path::end())
    .and(crate::helpers::deserialize())
    .and_then(
        move |auth: AppResult<(
            Option<models::User>,
            Option<models::KitMembership>,
            models::Kit,
            models::KitConfiguration,
        )>,
              peripheral: Peripheral| {
            let pg = pg.clone();
            async move {
                match auth {
                    Ok((_, _, kit, configuration)) => {
                        implementation(pg, kit, configuration, peripheral)
                            .never_error()
                            .await
                    }
                    Err(err) => Ok(Err(err)),
                }
            }
        },
    )
}

/// Handles the `PATCH` and `DELETE /kit-configurations/{kitConfigurationId}/peripherals/{peripheralId}?kitSerial={kitSerial}`
/// route.
fn patch_or_delete_peripheral(
    pg: PgPool,
) -> impl Filter<Extract = (AppResult<Response>,), Error = Rejection> + Clone {
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
            .and(pg.clone().filter())
            .and_then(|peripheral_id: models::PeripheralId, conn: PgPooled| {
                helpers::threadpool(move || {
                    match models::Peripheral::by_id(&conn, peripheral_id)
                        .map_err(|_| warp::reject::custom(problem::INTERNAL_SERVER_ERROR))?
                    {
                        Some(peripheral) => Ok(Ok(peripheral)),
                        None => Ok(Err(warp::reject::custom(problem::NOT_FOUND))),
                    }
                })
                .map(helpers::flatten_result)
            }),
    )
    .and(warp::path::end())
    .and_then(
        |auth: AppResult<(
            Option<models::User>,
            Option<models::KitMembership>,
            models::Kit,
            models::KitConfiguration,
        )>,
         peripheral: models::Peripheral| {
            async {
                match auth {
                    Ok((_, _, _, configuration)) => {
                        helpers::guard((configuration, peripheral), |(configuration, _)| {
                            if configuration.never_used {
                                None
                            } else {
                                Some(warp::reject::custom(
                                    problem::InvalidParameterReason::AlreadyActivated
                                        .singleton("configurationId")
                                        .into_problem(),
                                ))
                            }
                        })
                    }
                    Err(err) => Err(warp::reject::custom(err)),
                }
            }
        },
    )
    .untuple_one();

    async fn patch_implementation(
        pg: PgPool,
        peripheral: models::Peripheral,
        peripheral_patch: PeripheralPatch,
    ) -> AppResult<Response> {
        let conn = pg.get().await?;
        let updated_peripheral = helpers::threadpool(move || {
            conn.transaction(|| {
                let patched_peripheral = models::UpdatePeripheral {
                    id: peripheral.id,
                    name: peripheral_patch.name,
                    configuration: peripheral_patch.configuration,
                };

                if let Err(validation_errors) = patched_peripheral.validate() {
                    let invalid_parameters = problem::InvalidParameters::from(validation_errors);
                    return Err(invalid_parameters.into_problem());
                }

                let definition = match models::PeripheralDefinition::by_id(
                    &conn,
                    peripheral.peripheral_definition_id,
                )
                .optional()?
                {
                    Some(definition) => definition,
                    None => return Err(problem::INTERNAL_SERVER_ERROR),
                };

                if let Some(configuration) = patched_peripheral.configuration.as_ref() {
                    if let Err(problem) = check_configuration(configuration, &definition) {
                        return Err(problem);
                    }
                }

                Ok(patched_peripheral.update(&conn)?)
            })
        })
        .await?;
        Ok(ResponseBuilder::ok().body(views::Peripheral::from(updated_peripheral)))
    }

    async fn delete_implementation(
        pg: PgPool,
        peripheral: models::Peripheral,
    ) -> AppResult<Response> {
        let conn = pg.get().await?;
        helpers::threadpool(move || {
            peripheral.delete(&conn)?;
            Ok(ResponseBuilder::ok().empty())
        })
        .await
    }

    let pg2 = pg.clone();
    (base
        .clone()
        .and(warp::patch())
        .and(crate::helpers::deserialize())
        .and_then(
            move |_configuration: models::KitConfiguration,
                  peripheral: models::Peripheral,
                  peripheral_patch: PeripheralPatch| {
                patch_implementation(pg.clone(), peripheral, peripheral_patch).never_error()
            },
        ))
    .or(base.and(warp::delete()).and_then(
        move |_configuration: models::KitConfiguration, peripheral: models::Peripheral| {
            delete_implementation(pg2.clone(), peripheral).never_error()
        },
    ))
    .unify()
}
