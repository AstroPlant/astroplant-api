use axum::{extract::Path, Extension};

use serde::Deserialize;

use astroplant_mqtt::KitsRpc;

use crate::database::PgPool;
use crate::problem::{self, Problem};
use crate::response::{Response, ResponseBuilder};
use crate::{helpers, models};

/// Handles the `GET /kit-rpc/{kitSerial}/version` route.
pub async fn version(
    Extension(kits_rpc): Extension<KitsRpc>,
    Extension(pg): Extension<PgPool>,
    Path(kit_serial): Path<String>,
    user_id: Option<models::UserId>,
) -> Result<Response, Problem> {
    println!("HERE");
    let kits_rpc = kits_rpc.clone();
    let (_, _, kit) = helpers::fut_kit_permission_or_forbidden(
        pg,
        user_id,
        kit_serial,
        crate::authorization::KitAction::RpcVersion,
    )
    .await?;
    let version = kits_rpc
        .version(kit.serial)
        .await
        .map_err(problem::KitRpcProblem::kit_rpc_response_error_into_problem)?;
    Ok(ResponseBuilder::ok().body(version))
}

/// Handles the `GET /kit-rpc/{kitSerial}/uptime` route.
pub async fn uptime(
    Extension(kits_rpc): Extension<KitsRpc>,
    Extension(pg): Extension<PgPool>,
    Path(kit_serial): Path<String>,
    user_id: Option<models::UserId>,
) -> Result<Response, Problem> {
    let kits_rpc = kits_rpc.clone();
    let (_, _, kit) = helpers::fut_kit_permission_or_forbidden(
        pg,
        user_id,
        kit_serial,
        crate::authorization::KitAction::RpcUptime,
    )
    .await?;
    let uptime = kits_rpc
        .uptime(kit.serial)
        .await
        .map_err(problem::KitRpcProblem::kit_rpc_response_error_into_problem)?;
    Ok(ResponseBuilder::ok().body(uptime.as_secs()))
}

#[derive(Deserialize)]
pub struct PeripheralCommand {
    peripheral: String,
    command: serde_json::Value,
}

/// Handles the `POST /kit-rpc/{kitSerial}/peripheral-command` route.
pub async fn peripheral_command(
    Extension(kits_rpc): Extension<KitsRpc>,
    Extension(pg): Extension<PgPool>,
    Path(kit_serial): Path<String>,
    user_id: Option<models::UserId>,
    crate::extract::Json(peripheral_command): crate::extract::Json<PeripheralCommand>,
) -> Result<Response, Problem> {
    let kits_rpc = kits_rpc.clone();
    let (_, _, kit) = helpers::fut_kit_permission_or_forbidden(
        pg,
        user_id,
        kit_serial,
        crate::authorization::KitAction::RpcPeripheralCommand,
    )
    .await?;
    let peripheral_command = kits_rpc
        .peripheral_command(
            kit.serial,
            peripheral_command.peripheral,
            peripheral_command.command,
        )
        .await
        .map_err(problem::KitRpcProblem::kit_rpc_response_error_into_problem)?;
    Ok(ResponseBuilder::ok().data(peripheral_command.media_type, peripheral_command.data))
}
