use futures::future::FutureExt;
use serde::Deserialize;
use warp::{filters::BoxedFilter, Filter, Rejection};

use crate::authorization::{KitUser, Permission};
use crate::database::PgPool;
use crate::problem::{self, AppResult, Problem};
use crate::response::{Response, ResponseBuilder};
use crate::{authentication, helpers, models};

pub fn router(pg: PgPool) -> BoxedFilter<(AppResult<Response>,)> {
    //impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    tracing::trace!("Setting up permissions router.");

    warp::path::end()
        .and(user_kit_permissions(pg.clone()))
        .boxed()
}

/// Handles the `GET /permissions/?kitSerial={kitSerial}` route.
pub fn user_kit_permissions(
    pg: PgPool,
) -> impl Filter<Extract = (AppResult<Response>,), Error = Rejection> + Clone {
    use diesel::Connection;

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct KitSerial {
        kit_serial: String,
    }

    async fn implementation(
        pg: PgPool,
        user_id: Option<models::UserId>,
        kit_serial: KitSerial,
    ) -> AppResult<Response> {
        use crate::authorization::KitAction;
        use strum::IntoEnumIterator;

        let conn = pg.get().await?;
        let (user, membership, kit) = helpers::threadpool(move || {
            conn.transaction(|| {
                let user = if let Some(user_id) = user_id {
                    models::User::by_id(&conn, user_id)?
                } else {
                    None
                };

                let kit = models::Kit::by_serial(&conn, kit_serial.kit_serial)?;
                if kit.is_none() {
                    return Ok(None);
                }
                let kit = kit.unwrap();

                let membership = if let Some(user_id) = user_id {
                    models::KitMembership::by_user_id_and_kit_id(&conn, user_id, kit.get_id())?
                } else {
                    None
                };

                Ok::<_, Problem>(Some((user, membership, kit)))
            })
        })
        .await?
        .ok_or_else(|| problem::NOT_FOUND)?;

        let kit_user = match (user, membership) {
            (None, _) => KitUser::Anonymous,
            (Some(user), None) => KitUser::User(user),
            (Some(user), Some(kit_membership)) => KitUser::UserWithMembership(user, kit_membership),
        };

        let permissions: Vec<KitAction> = KitAction::iter()
            .filter(|action| action.permitted(&kit_user, &kit))
            .collect();

        Ok(ResponseBuilder::ok().body(permissions))
    }

    authentication::option_by_token()
        .and(warp::query::query::<KitSerial>())
        .and_then(
            move |user_id: Option<models::UserId>, kit_serial: KitSerial| {
                implementation(pg.clone(), user_id, kit_serial).never_error()
            },
        )
}
