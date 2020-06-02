mod peripheral;

use futures::FutureExt;
use serde::Deserialize;
use warp::{filters::BoxedFilter, path, Filter, Rejection};

use crate::database::PgPool;
use crate::problem::{self, AppResult, Problem};
use crate::response::{Response, ResponseBuilder};
use crate::utils::deserialize_some;
use crate::{helpers, models, views};

pub fn router(pg: PgPool) -> BoxedFilter<(AppResult<Response>,)> {
    //impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    trace!("Setting up configurations router.");

    configurations_by_kit_serial(pg.clone())
        .or(create_configuration(pg.clone()))
        .unify()
        .or(patch_configuration(pg.clone()))
        .unify()
        .or(peripheral::router(pg))
        .unify()
        .boxed()
}

/// Get the kit configuration from the path or error with 404.
fn get_kit_configuration(pg: PgPool) -> BoxedFilter<(AppResult<models::KitConfiguration>,)> {
    async fn implementation(
        pg: PgPool,
        configuration_id: i32,
    ) -> AppResult<models::KitConfiguration> {
        let conn = pg.get().await?;
        helpers::threadpool(move || {
            models::KitConfiguration::by_id(&conn, models::KitConfigurationId(configuration_id))
        })
        .await?
        .ok_or_else(|| problem::NOT_FOUND)
    }

    path!(i32 / ..)
        .and_then(move |configuration_id: i32| {
            implementation(pg.clone(), configuration_id).never_error()
        })
        .boxed()
}

/// Authorize the user against the kit in the query for the given action, get the kit configuration
/// from the path, and compare the configuration's kit id and with the kit's id. Rejects the request
/// on error.
fn authorize_and_get_kit_configuration(
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
    async fn implementation(
        kit_configuration: AppResult<models::KitConfiguration>,
        user: Option<models::User>,
        kit_membership: Option<models::KitMembership>,
        kit: models::Kit,
    ) -> AppResult<(
        Option<models::User>,
        Option<models::KitMembership>,
        models::Kit,
        models::KitConfiguration,
    )> {
        let kit_configuration = kit_configuration?;
        if kit_configuration.kit_id == kit.id {
            Ok((user, kit_membership, kit, kit_configuration))
        } else {
            Err(problem::NOT_FOUND)
        }
    }

    get_kit_configuration(pg.clone())
        .and(helpers::authorization_user_kit_from_query(
            pg.clone(),
            action,
        ))
        .and_then(
            |kit_configuration: AppResult<models::KitConfiguration>,
             user: Option<models::User>,
             kit_membership: Option<models::KitMembership>,
             kit: models::Kit| {
                implementation(kit_configuration, user, kit_membership, kit).never_error()
            },
        )
        .boxed()
}

/// Handles the `GET /kit-configurations?kitSerial={kitSerial}` route.
fn configurations_by_kit_serial(
    pg: PgPool,
) -> impl Filter<Extract = (AppResult<Response>,), Error = Rejection> + Clone {
    use diesel::Connection;
    use itertools::Itertools;
    use std::collections::HashMap;

    async fn implementation(pg: PgPool, kit: models::Kit) -> AppResult<Response> {
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
        .and(warp::path::end())
        .and(
            helpers::authorization_user_kit_from_query(
                pg.clone(),
                crate::authorization::KitAction::View,
            )
            .map(|_, _, kit| kit),
        )
        .and_then(move |kit| implementation(pg.clone(), kit).never_error())
}

/// Handles the `POST /kit-configurations?kitSerial={kitSerial}` route.
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
        kit: models::Kit,
        configuration: Configuration,
    ) -> AppResult<Response> {
        let conn = pg.get().await?;
        let new_configuration =
            models::NewKitConfiguration::new(kit.get_id(), configuration.description);
        let created_configuration =
            helpers::threadpool(move || new_configuration.create(&conn)).await?;
        Ok(ResponseBuilder::ok().body(views::KitConfiguration::from(created_configuration)))
    }

    warp::post()
        .and(warp::path::end())
        .and(
            helpers::authorization_user_kit_from_query(
                pg.clone(),
                crate::authorization::KitAction::EditConfiguration,
            )
            .map(|_, _, kit| kit),
        )
        .and(crate::helpers::deserialize())
        .and_then(move |kit: models::Kit, configuration: Configuration| {
            implementation(pg.clone(), kit, configuration).never_error()
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
        rules_supervisor_module_name: Option<String>,
        rules_supervisor_class_name: Option<String>,
        rules: Option<serde_json::Value>,
        active: Option<bool>,
    }

    async fn implementation(
        pg: PgPool,
        kit: models::Kit,
        configuration: models::KitConfiguration,
        configuration_patch: KitConfigurationPatch,
    ) -> AppResult<Response> {
        if !configuration.never_used {
            if configuration_patch.rules_supervisor_module_name.is_some()
                || configuration_patch.rules_supervisor_class_name.is_some()
                || configuration_patch.rules.is_some()
            {
                return Err(problem::InvalidParameterReason::AlreadyActivated
                    .singleton("configurationId")
                    .into_problem());
            }
        }

        let patch = models::UpdateKitConfiguration {
            id: configuration.id,
            description: configuration_patch.description,
            rules_supervisor_module_name: configuration_patch.rules_supervisor_module_name,
            rules_supervisor_class_name: configuration_patch.rules_supervisor_class_name,
            rules: configuration_patch.rules,
            active: configuration_patch.active,
            never_used: match configuration_patch.active {
                Some(true) => Some(false),
                _ => None,
            },
        };

        let conn = pg.get().await?;
        let patched_configuration = helpers::threadpool(move || {
            conn.transaction(|| {
                if let Some(active) = patch.active {
                    if active != configuration.active {
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
        .and(authorize_and_get_kit_configuration(
            pg.clone(),
            crate::authorization::KitAction::EditConfiguration,
        ))
        .and(warp::path::end())
        .and(crate::helpers::deserialize())
        .and_then(
            move |auth: AppResult<(
                Option<models::User>,
                Option<models::KitMembership>,
                models::Kit,
                models::KitConfiguration,
            )>,
                  configuration_patch: KitConfigurationPatch| {
                let pg = pg.clone();
                async move {
                    match auth {
                        Ok((_, _, kit, configuration)) => {
                            implementation(pg, kit, configuration, configuration_patch)
                                .never_error()
                                .await
                        }
                        Err(err) => Ok(Err(err)),
                    }
                }
            },
        )
}
