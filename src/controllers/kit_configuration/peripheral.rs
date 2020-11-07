use futures::future::FutureExt;
use serde::Deserialize;
use validator::Validate;
use warp::{filters::BoxedFilter, Filter, Rejection};

use crate::database::PgPool;
use crate::problem::{self, AppResult};
use crate::response::{Response, ResponseBuilder};
use crate::{authentication, authorization, helpers, models, views};

pub fn router(pg: PgPool) -> BoxedFilter<(AppResult<Response>,)> {
    //impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    tracing::trace!("Setting up peripherals router.");

    add_peripheral_to_configuration(pg.clone())
        .or(patch_or_delete_peripheral(pg.clone()))
        .unify()
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
                tracing::error!(
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

/// Handles the `POST /kit-configurations/{kitConfigurationId}/peripherals` route.
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
        user_id: Option<models::UserId>,
        kit_configuration_id: models::KitConfigurationId,
        peripheral: Peripheral,
    ) -> AppResult<Response> {
        let (kit, configuration) =
            super::get_models_from_kit_configuration_id(pg.clone(), kit_configuration_id).await?;
        super::authorize(
            pg.clone(),
            user_id,
            &kit,
            authorization::KitAction::EditConfiguration,
        )
        .await?;

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

    warp::post()
        .and(warp::path!("kit-configurations" / i32 / "peripherals"))
        .and(authentication::option_by_token())
        .and(crate::helpers::deserialize())
        .and_then(
            move |kit_configuration_id: i32, user_id, peripheral: Peripheral| {
                implementation(
                    pg.clone(),
                    user_id,
                    models::KitConfigurationId(kit_configuration_id),
                    peripheral,
                )
                .never_error()
            },
        )
}

/// Handles the `PATCH` and `DELETE /peripherals/{peripheralId}` routes.
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

    /// Check user authorization and make sure the configuration has never been activated.
    async fn base(
        pg: PgPool,
        user_id: Option<models::UserId>,
        peripheral_id: models::PeripheralId,
    ) -> AppResult<(models::Kit, models::KitConfiguration, models::Peripheral)> {
        let (kit, kit_configuration, peripheral) =
            super::get_models_from_peripheral_id(pg.clone(), peripheral_id).await?;
        super::authorize(
            pg.clone(),
            user_id,
            &kit,
            authorization::KitAction::EditConfiguration,
        )
        .await?;

        if !kit_configuration.never_used {
            return Err(problem::InvalidParameterReason::AlreadyActivated
                .singleton("configurationId")
                .into_problem());
        }

        Ok((kit, kit_configuration, peripheral))
    }

    async fn patch_implementation(
        pg: PgPool,
        user_id: Option<models::UserId>,
        peripheral_id: models::PeripheralId,
        peripheral_patch: PeripheralPatch,
    ) -> AppResult<Response> {
        let (_, _, peripheral) = base(pg.clone(), user_id, peripheral_id).await?;

        let patched_peripheral = models::UpdatePeripheral {
            id: peripheral.id,
            name: peripheral_patch.name,
            configuration: peripheral_patch.configuration,
        };

        if let Err(validation_errors) = patched_peripheral.validate() {
            let invalid_parameters = problem::InvalidParameters::from(validation_errors);
            return Err(invalid_parameters.into_problem());
        }

        let conn = pg.get().await?;
        let updated_peripheral = helpers::threadpool(move || {
            conn.transaction(|| {
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
        user_id: Option<models::UserId>,
        peripheral_id: models::PeripheralId,
    ) -> AppResult<Response> {
        let (_, _, peripheral) = base(pg.clone(), user_id, peripheral_id).await?;

        let conn = pg.get().await?;
        helpers::threadpool(move || {
            peripheral.delete(&conn)?;
            Ok(ResponseBuilder::ok().empty())
        })
        .await
    }

    let pg2 = pg.clone();
    (warp::patch()
        .and(warp::path!("peripherals" / i32))
        .and(authentication::option_by_token())
        .and(crate::helpers::deserialize())
        .and_then(
            move |peripheral_id: i32,
                  user_id: Option<models::UserId>,
                  peripheral_patch: PeripheralPatch| {
                patch_implementation(
                    pg.clone(),
                    user_id,
                    models::PeripheralId(peripheral_id),
                    peripheral_patch,
                )
                .never_error()
            },
        ))
    .or(warp::delete()
        .and(warp::path!("peripherals" / i32))
        .and(authentication::option_by_token())
        .and_then(move |peripheral_id: i32, user_id: Option<models::UserId>| {
            delete_implementation(pg2.clone(), user_id, models::PeripheralId(peripheral_id))
                .never_error()
        }))
    .unify()
}
