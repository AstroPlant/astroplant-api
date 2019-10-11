use serde::Deserialize;
use validator::Validate;
use warp::{filters::BoxedFilter, path, Filter, Rejection};

use crate::response::{Response, ResponseBuilder};
use crate::PgPooled;
use crate::{helpers, models, problem, views};

pub fn router(pg: BoxedFilter<(crate::PgPooled,)>) -> BoxedFilter<(Response,)> {
    //impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    trace!("Setting up configurations/peripheral router.");

    add_peripheral_to_configuration(pg.clone()).boxed()
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

/// Handles the `POST /kit-configurations/{kitConfigurationId}/peripherals?kitSerial={kitSerial}`
/// route.
fn add_peripheral_to_configuration(
    pg: BoxedFilter<(crate::PgPooled,)>,
) -> impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    use diesel::Connection;
    use futures::future::Future;

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
                        return Ok(Err(warp::reject::custom(
                            problem::Problem::InvalidParameters { invalid_parameters },
                        )));
                    }

                    let definition = models::PeripheralDefinition::by_id(
                        &conn,
                        peripheral.peripheral_definition_id,
                    )?;

                    let mut scope = valico::json_schema::Scope::new();
                    let schema =
                        match scope.compile_and_return(definition.configuration_schema, false) {
                            Ok(schema) => schema,
                            Err(_) => return Ok(Err(warp::reject::custom(problem::NOT_FOUND))),
                        };

                    let mut invalid_parameters = problem::InvalidParameters::new();
                    if !schema
                        .validate(&new_peripheral.configuration)
                        .is_strictly_valid()
                    {
                        invalid_parameters
                            .add("configuration", problem::InvalidParameterReason::Other)
                    }

                    if !invalid_parameters.is_empty() {
                        return Ok(Err(warp::reject::custom(
                            problem::Problem::InvalidParameters { invalid_parameters },
                        )));
                    }

                    let created_peripheral = new_peripheral.create(&conn)?;

                    Ok(Ok(
                        ResponseBuilder::ok().body(views::Peripheral::from(created_peripheral))
                    ))
                })
                //.body(views::KitConfiguration::from(patched_configuration)))
            })
            .then(helpers::flatten_result)
        },
    )
}
