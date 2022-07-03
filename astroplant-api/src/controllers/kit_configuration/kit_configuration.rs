use axum::extract::Path;
use axum::Extension;
use serde::Deserialize;

use crate::database::PgPool;
use crate::problem::{self, Problem};
use crate::response::{Response, ResponseBuilder};
use crate::utils::deserialize_some;
use crate::{authorization, helpers, models, views};

/// Handles the `GET /kits/{kitSerial}/configurations` route.
pub async fn configurations_by_kit_serial(
    Extension(pg): Extension<PgPool>,
    user_id: Option<models::UserId>,
    Path(kit_serial): Path<String>,
) -> Result<Response, Problem> {
    use diesel::Connection;
    use itertools::Itertools;
    use std::collections::HashMap;

    let (_user, _membership, kit) = helpers::fut_kit_permission_or_forbidden(
        pg.clone(),
        user_id,
        kit_serial,
        authorization::KitAction::View,
    )
    .await?;

    let conn = pg.get().await?;
    helpers::threadpool(move || {
        conn.transaction(|| {
            let kit_configurations = models::KitConfiguration::configurations_of_kit(&conn, &kit)?;
            let kit_peripherals = models::Peripheral::peripherals_of_kit(&conn, &kit)?;
            let mut kit_peripherals: HashMap<i32, Vec<views::Peripheral>> = kit_peripherals
                .into_iter()
                .map(|p| (p.kit_configuration_id, views::Peripheral::from(p)))
                .into_group_map();
            let kit_configurations_with_peripherals: Vec<
                views::KitConfigurationWithPeripherals<views::Peripheral>,
            > = kit_configurations
                .into_iter()
                .map(views::KitConfiguration::from)
                .map(|c| {
                    let id = c.id;
                    c.with_peripherals(kit_peripherals.remove(&id).unwrap_or_default())
                })
                .collect();
            Ok(ResponseBuilder::ok().body(kit_configurations_with_peripherals))
        })
    })
    .await
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Configuration {
    description: Option<String>,
}

/// Handles the `POST /kits/{kitSerial}/configurations` route.
pub async fn create_configuration(
    Extension(pg): Extension<PgPool>,
    user_id: Option<models::UserId>,
    Path(kit_serial): Path<String>,
    crate::extract::Json(configuration): crate::extract::Json<Configuration>,
) -> Result<Response, Problem> {
    let (_user, _membership, kit) = helpers::fut_kit_permission_or_forbidden(
        pg.clone(),
        user_id,
        kit_serial,
        authorization::KitAction::EditConfiguration,
    )
    .await?;

    let conn = pg.get().await?;
    let new_configuration =
        models::NewKitConfiguration::new(kit.get_id(), configuration.description);
    let created_configuration =
        helpers::threadpool(move || new_configuration.create(&conn)).await?;
    Ok(ResponseBuilder::ok().body(views::KitConfiguration::from(created_configuration)))
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct KitConfigurationPatch {
    #[serde(default, deserialize_with = "deserialize_some")]
    description: Option<Option<String>>,
    controller_symbol_location: Option<String>,
    controller_symbol: Option<String>,
    control_rules: Option<serde_json::Value>,
    active: Option<bool>,
}

/// Handles the `PATCH /kit-configurations/{kitConfigurationId}` route.
///
/// If the configuration is set active, all other configurations of the kit are deactivated.
pub async fn patch_configuration(
    Extension(pg): Extension<PgPool>,
    user_id: Option<models::UserId>,
    Path(kit_configuration_id): Path<i32>,
    crate::extract::Json(kit_configuration_patch): crate::extract::Json<KitConfigurationPatch>,
) -> Result<Response, Problem> {
    use diesel::Connection;

    let kit_configuration_id = models::KitConfigurationId(kit_configuration_id);

    let (kit, kit_configuration) =
        super::get_models_from_kit_configuration_id(pg.clone(), kit_configuration_id).await?;
    super::authorize(
        pg.clone(),
        user_id,
        &kit,
        authorization::KitAction::EditConfiguration,
    )
    .await?;

    if !kit_configuration.never_used
        && (kit_configuration_patch.controller_symbol_location.is_some()
            || kit_configuration_patch.controller_symbol.is_some()
            || kit_configuration_patch.control_rules.is_some())
    {
        // Cannot change rules or rules supervisor if configuration has already been activated.
        return Err(problem::InvalidParameterReason::AlreadyActivated
            .singleton("configurationId")
            .into_problem());
    }

    let patch = models::UpdateKitConfiguration {
        id: kit_configuration.id,
        description: kit_configuration_patch.description,
        controller_symbol_location: kit_configuration_patch.controller_symbol_location,
        controller_symbol: kit_configuration_patch.controller_symbol,
        control_rules: kit_configuration_patch.control_rules,
        active: kit_configuration_patch.active,
        never_used: match kit_configuration_patch.active {
            Some(true) => Some(false),
            _ => None,
        },
    };

    let conn = pg.get().await?;
    let patched_configuration = helpers::threadpool(move || {
        conn.transaction(|| {
            if let Some(active) = patch.active {
                if active != kit_configuration.active {
                    models::KitConfiguration::deactivate_all_of_kit(&conn, &kit)?;
                }
            }
            Ok::<_, Problem>(patch.update(&conn)?)
        })
    })
    .await?;

    Ok(ResponseBuilder::ok().body(views::KitConfiguration::from(patched_configuration)))
}
