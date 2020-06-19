use futures::FutureExt;
use serde::Deserialize;
use warp::{filters::BoxedFilter, Filter, Rejection};

use crate::database::PgPool;
use crate::problem::{self, AppResult, Problem};
use crate::response::{Response, ResponseBuilder};
use crate::utils::deserialize_some;
use crate::{authentication, authorization, helpers, models, views};

pub fn router(pg: PgPool) -> BoxedFilter<(AppResult<Response>,)> {
    //impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    trace!("Setting up configurations router.");

    configurations_by_kit_serial(pg.clone())
        .or(create_configuration(pg.clone()))
        .unify()
        .or(patch_configuration(pg.clone()))
        .unify()
        .boxed()
}

/// Handles the `GET /kits/{kitSerial}/configurations` route.
fn configurations_by_kit_serial(
    pg: PgPool,
) -> impl Filter<Extract = (AppResult<Response>,), Error = Rejection> + Clone {
    use diesel::Connection;
    use itertools::Itertools;
    use std::collections::HashMap;

    async fn implementation(
        pg: PgPool,
        user_id: Option<models::UserId>,
        kit_serial: String,
    ) -> AppResult<Response> {
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
                        c.with_peripherals(kit_peripherals.remove(&id).unwrap_or_else(|| vec![]))
                    })
                    .collect();
                Ok(ResponseBuilder::ok().body(kit_configurations_with_peripherals))
            })
        })
        .await
    }

    warp::get()
        .and(warp::path!("kits" / String / "configurations"))
        .and(authentication::option_by_token())
        .and_then(move |kit_serial, user_id| {
            implementation(pg.clone(), user_id, kit_serial).never_error()
        })
}

/// Handles the `POST /kits/{kitSerial}/configurations` route.
fn create_configuration(
    pg: PgPool,
) -> impl Filter<Extract = (AppResult<Response>,), Error = Rejection> + Clone {
    #[derive(Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    struct Configuration {
        description: Option<String>,
    }

    async fn implementation(
        pg: PgPool,
        user_id: Option<models::UserId>,
        kit_serial: String,
        configuration: Configuration,
    ) -> AppResult<Response> {
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

    warp::post()
        .and(warp::path!("kits" / String / "configurations"))
        .and(authentication::option_by_token())
        .and(crate::helpers::deserialize())
        .and_then(move |kit_serial, user_id, configuration| {
            implementation(pg.clone(), user_id, kit_serial, configuration).never_error()
        })
}

/// Handles the `PATCH /kit-configurations/{kitConfigurationId}?kitSerial={kitSerial}` route.
///
/// If the configuration is set active, all other configurations of the kit are deactivated.
fn patch_configuration(
    pg: PgPool,
) -> impl Filter<Extract = (AppResult<Response>,), Error = Rejection> + Clone {
    use diesel::Connection;

    #[derive(Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    struct KitConfigurationPatch {
        #[serde(default, deserialize_with = "deserialize_some")]
        description: Option<Option<String>>,
        controller_symbol_location: Option<String>,
        controller_symbol: Option<String>,
        control_rules: Option<serde_json::Value>,
        active: Option<bool>,
    }

    async fn implementation(
        pg: PgPool,
        user_id: Option<models::UserId>,
        kit_configuration_id: models::KitConfigurationId,
        kit_configuration_patch: KitConfigurationPatch,
    ) -> AppResult<Response> {
        let (kit, kit_configuration) =
            super::get_models_from_kit_configuration_id(pg.clone(), kit_configuration_id).await?;
        super::authorize(
            pg.clone(),
            user_id,
            &kit,
            authorization::KitAction::EditConfiguration,
        )
        .await?;

        if !kit_configuration.never_used {
            if kit_configuration_patch.controller_symbol_location.is_some()
                || kit_configuration_patch.controller_symbol.is_some()
                || kit_configuration_patch.control_rules.is_some()
            {
                // Cannot change rules or rules supervisor if configuration has already been activated.
                return Err(problem::InvalidParameterReason::AlreadyActivated
                    .singleton("configurationId")
                    .into_problem());
            }
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

    warp::patch()
        .and(warp::path!("kit-configurations" / i32))
        .and(authentication::option_by_token())
        .and(crate::helpers::deserialize())
        .and_then(
            move |kit_configuration_id, user_id, kit_configuration_patch| {
                implementation(
                    pg.clone(),
                    user_id,
                    models::KitConfigurationId(kit_configuration_id),
                    kit_configuration_patch,
                )
                .never_error()
            },
        )
}
