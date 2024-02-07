use axum::extract::Path;
use axum::Extension;

use serde::Deserialize;
use validator::Validate;

use crate::database::PgPool;
use crate::problem::{self, AppResult, Problem};
use crate::response::{Response, ResponseBuilder};
use crate::{authorization, models, views};

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

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Peripheral {
    peripheral_definition_id: i32,
    name: String,
    configuration: serde_json::Value,
}

/// Handles the `POST /kit-configurations/{kitConfigurationId}/peripherals` route.
// TODO: ensure peripheral names are unique per configuration
pub async fn add_peripheral_to_configuration(
    Extension(pg): Extension<PgPool>,
    user_id: Option<models::UserId>,
    Path(kit_configuration_id): Path<i32>,
    crate::extract::Json(peripheral): crate::extract::Json<Peripheral>,
) -> Result<Response, Problem> {
    use diesel::prelude::*;

    let kit_configuration_id = models::KitConfigurationId(kit_configuration_id);

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
    let created_peripheral = conn
        .interact_flatten_err(move |conn| {
            conn.transaction(|conn| {
                let definition =
                    match models::PeripheralDefinition::by_id(conn, peripheral_definition_id)
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

                Ok(new_peripheral.create(conn)?)
            })
        })
        .await?;
    Ok(ResponseBuilder::ok().body(views::Peripheral::from(created_peripheral)))
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PeripheralPatch {
    name: Option<String>,
    configuration: Option<serde_json::Value>,
}

/// Handles the `PATCH /peripherals/{peripheralId}` routes.
pub async fn patch_peripheral(
    Extension(pg): Extension<PgPool>,
    user_id: Option<models::UserId>,
    Path(peripheral_id): Path<i32>,
    crate::extract::Json(peripheral_patch): crate::extract::Json<PeripheralPatch>,
) -> Result<Response, Problem> {
    use diesel::prelude::*;

    let peripheral_id = models::PeripheralId(peripheral_id);

    // Check user authorization and make sure the configuration has never been activated.
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
    let updated_peripheral = conn
        .interact_flatten_err(move |conn| {
            conn.transaction(|conn| {
                let definition = match models::PeripheralDefinition::by_id(
                    conn,
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

                Ok(patched_peripheral.update(conn)?)
            })
        })
        .await?;
    Ok(ResponseBuilder::ok().body(views::Peripheral::from(updated_peripheral)))
}

/// Handles the `DELETE /peripherals/{peripheralId}` routes.
pub async fn delete_peripheral(
    Extension(pg): Extension<PgPool>,
    user_id: Option<models::UserId>,
    Path(peripheral_id): Path<i32>,
) -> Result<Response, Problem> {
    let peripheral_id = models::PeripheralId(peripheral_id);

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

    let conn = pg.get().await?;
    conn.interact(move |conn| {
        peripheral.delete(conn)?;
        Ok(ResponseBuilder::ok().empty())
    })
    .await?
}
