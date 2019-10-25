use futures::future::FutureExt;
use serde::Deserialize;
use warp::{filters::BoxedFilter, Filter, Rejection};

use crate::response::{Response, ResponseBuilder};
use crate::PgPooled;
use crate::{authentication, helpers, models, problem};

pub fn router(pg: BoxedFilter<(crate::PgPooled,)>) -> BoxedFilter<(Response,)> {
    //impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    trace!("Setting up permissions router.");

    warp::path::end()
        .and(user_kit_permissions(pg.clone().boxed()))
        .boxed()
}

/// Handles the `GET /permissions/?kitSerial={kitSerial}` route.
pub fn user_kit_permissions(
    pg: BoxedFilter<(crate::PgPooled,)>,
) -> impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    use diesel::Connection;

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct KitSerial {
        kit_serial: String,
    }

    authentication::option_by_token()
        .and(warp::query::query::<KitSerial>())
        .and(pg)
        .and_then(
            |user_id: Option<models::UserId>, kit_serial: KitSerial, conn: PgPooled| {
                helpers::threadpool_diesel_ok(move || {
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
                            models::KitMembership::by_user_id_and_kit_id(
                                &conn,
                                user_id,
                                kit.get_id(),
                            )?
                        } else {
                            None
                        };

                        Ok(Some((user, membership, kit)))
                    })
                })
                .map(move |result| match result {
                    Ok(None) => Err(warp::reject::custom(problem::NOT_FOUND)),
                    Ok(Some((user, membership, kit))) => {
                        use crate::authorization::KitAction;
                        use strum::IntoEnumIterator;

                        let permissions: Vec<KitAction> = KitAction::iter()
                            .filter(|action| action.permission(&user, &membership, &kit))
                            .collect();

                        Ok(ResponseBuilder::ok().body(permissions))
                    }
                    Err(e) => Err(e),
                })
            },
        )
}
