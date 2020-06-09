use astroplant_mqtt::KitsRpc;
use futures::future::FutureExt;
use serde::Deserialize;
use warp::{filters::BoxedFilter, path, Filter, Rejection};

use crate::database::PgPool;
use crate::problem::{self, AppResult};
use crate::response::{Response, ResponseBuilder};
use crate::{authentication, helpers, models};

pub fn router(kits_rpc: KitsRpc, pg: PgPool) -> BoxedFilter<(AppResult<Response>,)> {
    //impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    trace!("Setting up kit rpc router.");

    version(kits_rpc.clone(), pg.clone())
        .or(uptime(kits_rpc.clone(), pg.clone()))
        .unify()
        .or(peripheral_command(kits_rpc.clone(), pg.clone()))
        .unify()
        .boxed()
}

/// Handles the `GET /kit-rpc/{kitSerial}/version` route.
pub fn version(
    kits_rpc: KitsRpc,
    pg: PgPool,
) -> impl Filter<Extract = (AppResult<Response>,), Error = Rejection> + Clone {
    async fn implementation(
        kits_rpc: KitsRpc,
        pg: PgPool,
        kit_serial: String,
        user_id: Option<models::UserId>,
    ) -> AppResult<Response> {
        let kits_rpc = kits_rpc.clone();
        let (_, _, kit) = helpers::fut_kit_permission_or_forbidden(
            pg,
            user_id,
            kit_serial,
            crate::authorization::KitAction::RpcVersion,
        )
        .await?;
        let rpc = kits_rpc.kit_rpc(kit.serial);
        let version = rpc
            .version()
            .await
            .unwrap()
            .map_err(|err| problem::KitRpcProblem::kit_rpc_response_error_into_problem(err))?;
        Ok(ResponseBuilder::ok().body(version))
    }

    path!(String / "version")
        .and(authentication::option_by_token())
        .and_then(move |kit_serial: String, user_id: Option<models::UserId>| {
            implementation(kits_rpc.clone(), pg.clone(), kit_serial, user_id).never_error()
        })
}

/// Handles the `GET /kit-rpc/{kitSerial}/uptime` route.
pub fn uptime(
    kits_rpc: KitsRpc,
    pg: PgPool,
) -> impl Filter<Extract = (AppResult<Response>,), Error = Rejection> + Clone {
    async fn implementation(
        kits_rpc: KitsRpc,
        pg: PgPool,
        kit_serial: String,
        user_id: Option<models::UserId>,
    ) -> AppResult<Response> {
        let kits_rpc = kits_rpc.clone();
        let (_, _, kit) = helpers::fut_kit_permission_or_forbidden(
            pg,
            user_id,
            kit_serial,
            crate::authorization::KitAction::RpcUptime,
        )
        .await?;
        let rpc = kits_rpc.kit_rpc(kit.serial);
        let uptime = rpc
            .uptime()
            .await
            .unwrap()
            .map_err(|err| problem::KitRpcProblem::kit_rpc_response_error_into_problem(err))?;
        Ok(ResponseBuilder::ok().body(uptime.as_secs()))
    }

    path!(String / "uptime")
        .and(authentication::option_by_token())
        .and_then(move |kit_serial: String, user_id: Option<models::UserId>| {
            implementation(kits_rpc.clone(), pg.clone(), kit_serial, user_id).never_error()
        })
}

/// Handles the `POST /kit-rpc/{kitSerial}/peripheral-command` route.
pub fn peripheral_command(
    kits_rpc: KitsRpc,
    pg: PgPool,
) -> impl Filter<Extract = (AppResult<Response>,), Error = Rejection> + Clone {
    #[derive(Deserialize)]
    struct PeripheralCommand {
        peripheral: String,
        command: serde_json::Value,
    }

    async fn implementation(
        kits_rpc: KitsRpc,
        pg: PgPool,
        kit_serial: String,
        user_id: Option<models::UserId>,
        peripheral_command: PeripheralCommand,
    ) -> AppResult<Response> {
        let kits_rpc = kits_rpc.clone();
        let (_, _, kit) = helpers::fut_kit_permission_or_forbidden(
            pg,
            user_id,
            kit_serial,
            crate::authorization::KitAction::RpcPeripheralCommand,
        )
        .await?;
        let rpc = kits_rpc.kit_rpc(kit.serial);
        let peripheral_command = rpc
            .peripheral_command(peripheral_command.peripheral, peripheral_command.command)
            .await
            .unwrap()
            .map_err(|err| problem::KitRpcProblem::kit_rpc_response_error_into_problem(err))?;
        Ok(ResponseBuilder::ok().data(peripheral_command.media_type, peripheral_command.data))
    }

    path!(String / "peripheral-command")
        .and(authentication::option_by_token())
        .and(helpers::deserialize())
        .and_then(move |kit_serial, user_id, peripheral_command| {
            implementation(
                kits_rpc.clone(),
                pg.clone(),
                kit_serial,
                user_id,
                peripheral_command,
            )
            .never_error()
        })
}
