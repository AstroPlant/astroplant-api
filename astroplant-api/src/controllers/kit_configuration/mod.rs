mod kit_configuration;
mod peripheral;

pub use kit_configuration::{
    configurations_by_kit_serial, create_configuration, patch_configuration,
};
pub use peripheral::{add_peripheral_to_configuration, delete_peripheral, patch_peripheral};

use crate::database::PgPool;
use crate::problem::{self, AppResult};

use crate::{authorization, helpers, models};

async fn get_models_from_kit_configuration_id(
    pg: PgPool,
    kit_configuration_id: models::KitConfigurationId,
) -> AppResult<(models::Kit, models::KitConfiguration)> {
    let conn = pg.get().await?;
    conn.interact(move |conn| {
        let kit_configuration = models::KitConfiguration::by_id(conn, kit_configuration_id)?
            .ok_or(problem::NOT_FOUND)?;
        let kit = models::Kit::by_id(conn, kit_configuration.get_kit_id())?
            .ok_or(problem::INTERNAL_SERVER_ERROR)?;
        Ok((kit, kit_configuration))
    })
    .await?
}

async fn get_models_from_peripheral_id(
    pg: PgPool,
    peripheral_id: models::PeripheralId,
) -> AppResult<(models::Kit, models::KitConfiguration, models::Peripheral)> {
    let conn = pg.get().await?;
    conn.interact(move |conn| {
        let peripheral =
            models::Peripheral::by_id(conn, peripheral_id)?.ok_or(problem::NOT_FOUND)?;
        let kit = models::Kit::by_id(conn, peripheral.get_kit_id())?
            .ok_or(problem::INTERNAL_SERVER_ERROR)?;
        let configuration =
            models::KitConfiguration::by_id(conn, peripheral.get_kit_configuration_id())?
                .ok_or(problem::INTERNAL_SERVER_ERROR)?;
        Ok((kit, configuration, peripheral))
    })
    .await?
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
