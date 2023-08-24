use axum::Extension;

use serde::Deserialize;

use crate::authorization::{KitUser, Permission};
use crate::database::PgPool;
use crate::models;
use crate::problem::{self, Problem};
use crate::response::{Response, ResponseBuilder};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KitSerial {
    kit_serial: String,
}

/// Handles the `GET /permissions/?kitSerial={kitSerial}` route.
pub async fn user_kit_permissions(
    Extension(pg): Extension<PgPool>,
    user_id: Option<models::UserId>,
    crate::extract::Query(KitSerial { kit_serial }): crate::extract::Query<KitSerial>,
) -> Result<Response, Problem> {
    use diesel::Connection;

    use crate::authorization::KitAction;
    use strum::IntoEnumIterator;

    let conn = pg.get().await?;
    let (user, membership, kit) = conn
        .interact_flatten_err(move |conn| {
            conn.transaction(|conn| {
                let user = if let Some(user_id) = user_id {
                    models::User::by_id(conn, user_id)?
                } else {
                    None
                };

                let kit = models::Kit::by_serial(conn, kit_serial)?;
                if kit.is_none() {
                    return Ok(None);
                }
                let kit = kit.unwrap();

                let membership = if let Some(user_id) = user_id {
                    models::KitMembership::by_user_id_and_kit_id(conn, user_id, kit.get_id())?
                } else {
                    None
                };

                Ok::<_, Problem>(Some((user, membership, kit)))
            })
        })
        .await?
        .ok_or(problem::NOT_FOUND)?;

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
