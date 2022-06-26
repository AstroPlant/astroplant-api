mod kit_configuration;
mod peripheral;

use warp::{filters::BoxedFilter, Filter};

use crate::database::PgPool;
use crate::problem::{self, AppResult};
use crate::response::Response;
use crate::{authorization, helpers, models};

pub fn router(pg: PgPool) -> BoxedFilter<(AppResult<Response>,)> {
    //impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    tracing::trace!("Setting up kit configurations and peripherals router.");

    kit_configuration::router(pg.clone())
        .or(peripheral::router(pg))
        .unify()
        .boxed()
}

async fn get_models_from_kit_configuration_id(
    pg: PgPool,
    kit_configuration_id: models::KitConfigurationId,
) -> AppResult<(models::Kit, models::KitConfiguration)> {
    let conn = pg.get().await?;
    helpers::threadpool(move || {
        let kit_configuration = models::KitConfiguration::by_id(&conn, kit_configuration_id)?
            .ok_or_else(|| problem::NOT_FOUND)?;
        let kit = models::Kit::by_id(&conn, kit_configuration.get_kit_id())?
            .ok_or_else(|| problem::INTERNAL_SERVER_ERROR)?;
        Ok((kit, kit_configuration))
    })
    .await
}

async fn get_models_from_peripheral_id(
    pg: PgPool,
    peripheral_id: models::PeripheralId,
) -> AppResult<(models::Kit, models::KitConfiguration, models::Peripheral)> {
    let conn = pg.get().await?;
    helpers::threadpool(move || {
        let peripheral =
            models::Peripheral::by_id(&conn, peripheral_id)?.ok_or_else(|| problem::NOT_FOUND)?;
        let kit = models::Kit::by_id(&conn, peripheral.get_kit_id())?
            .ok_or_else(|| problem::INTERNAL_SERVER_ERROR)?;
        let configuration =
            models::KitConfiguration::by_id(&conn, peripheral.get_kit_configuration_id())?
                .ok_or_else(|| problem::INTERNAL_SERVER_ERROR)?;
        Ok((kit, configuration, peripheral))
    })
    .await
}

async fn authorize(
    pg: PgPool,
    user_id: Option<models::UserId>,
    kit: &models::Kit,
    action: authorization::KitAction,
) -> AppResult<(Option<models::User>, Option<models::KitMembership>)> {
    // FIXME: this unnecessarily queries for the kit: we already have it.
    let (user, membership, _) =
        helpers::fut_kit_permission_or_forbidden(pg, user_id, kit.serial.to_owned(), action)
            .await?;
    Ok((user, membership))
}
