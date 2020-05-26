use astroplant_mqtt::KitsRpc;
use futures::future::TryFutureExt;
use warp::{filters::BoxedFilter, path, Filter, Rejection};

use crate::response::{Response, ResponseBuilder};
use crate::PgPooled;
use crate::{authentication, helpers, models, problem};

pub fn router(kits_rpc: KitsRpc, pg: BoxedFilter<(crate::PgPooled,)>) -> BoxedFilter<(Response,)> {
    //impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    trace!("Setting up kit rpc router.");

    version(kits_rpc.clone(), pg.clone().boxed())
        .or(uptime(kits_rpc.clone(), pg.clone().boxed()))
        .unify()
        .boxed()
}

/// Handles the `GET /kit-rpc/{kitSerial}/version` route.
pub fn version(
    kits_rpc: KitsRpc,
    pg: BoxedFilter<(crate::PgPooled,)>,
) -> impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    path!(String / "version")
        .and(authentication::option_by_token())
        .and(pg.clone())
        .and_then(
            |kit_serial: String, user_id: Option<models::UserId>, conn: PgPooled| {
                helpers::fut_kit_permission_or_forbidden(
                    conn,
                    user_id,
                    kit_serial,
                    crate::authorization::KitAction::RpcVersion,
                )
                .map_ok(|(_, _, kit)| kit)
            },
        )
        .and_then(move |kit: models::Kit| {
            let kits_rpc = kits_rpc.clone();
            async move {
                let rpc = kits_rpc.kit_rpc(kit.serial);
                let version = rpc.version().await.unwrap().map_err(|err| {
                    warp::reject::custom(
                        problem::KitRpcProblem::kit_rpc_response_error_into_problem(err),
                    )
                })?;
                Ok::<_, Rejection>(ResponseBuilder::ok().body(version))
            }
        })
}

/// Handles the `GET /kit-rpc/{kitSerial}/uptime` route.
pub fn uptime(
    kits_rpc: KitsRpc,
    pg: BoxedFilter<(crate::PgPooled,)>,
) -> impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    path!(String / "uptime")
        .and(authentication::option_by_token())
        .and(pg.clone())
        .and_then(
            |kit_serial: String, user_id: Option<models::UserId>, conn: PgPooled| {
                helpers::fut_kit_permission_or_forbidden(
                    conn,
                    user_id,
                    kit_serial,
                    crate::authorization::KitAction::RpcUptime,
                )
                .map_ok(|(_, _, kit)| kit)
            },
        )
        .and_then(move |kit: models::Kit| {
            let kits_rpc = kits_rpc.clone();
            async move {
                let rpc = kits_rpc.kit_rpc(kit.serial);
                let uptime = rpc.uptime().await.unwrap().map_err(|err| {
                    warp::reject::custom(
                        problem::KitRpcProblem::kit_rpc_response_error_into_problem(err),
                    )
                })?;
                Ok::<_, Rejection>(ResponseBuilder::ok().body(uptime.as_secs()))
            }
        })
}
