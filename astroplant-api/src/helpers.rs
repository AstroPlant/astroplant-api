use futures::future::TryFutureExt;

use crate::authorization::{KitUser, Permission};
use crate::database::PgPool;
use crate::problem::{Problem, FORBIDDEN, INTERNAL_SERVER_ERROR, NOT_FOUND};

/// Run a blocking function on a threadpool.
pub async fn threadpool<F, T>(f: F) -> T
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(f).await.unwrap()
}

/// Runs a function on a threadpool, converting potential errors through Problem into Rejection.
pub async fn threadpool_result<F, T, E>(f: F) -> Result<T, Problem>
where
    F: FnOnce() -> Result<T, E> + Send + 'static,
    T: Send + 'static,
    E: Into<Problem> + Send + 'static + std::fmt::Debug,
{
    threadpool(f).await.map_err(|err| {
        tracing::error!("Error in threadpool: {:?}", err);
        err.into()
    })
}

#[allow(dead_code)]
pub fn some_or_internal_error<T>(r: Option<T>) -> Result<T, Problem> {
    r.ok_or(INTERNAL_SERVER_ERROR)
}

#[allow(dead_code)]
pub fn some_or_not_found<T>(r: Option<T>) -> Result<T, Problem> {
    r.ok_or(NOT_FOUND)
}

/**
 * Ensure the user has permission to perform the action on the kit.
 * Rejects the request otherwise.
 */
pub fn permission_or_forbidden<P>(
    actor: &P::Actor,
    object: &P::Object,
    permission: P,
) -> Result<(), Problem>
where
    P: Permission,
{
    if permission.permitted(actor, object) {
        Ok(())
    } else {
        Err(FORBIDDEN)
    }
}

/**
 * Ensure the user has permission to perform the action on the kit.
 * Rejects the request with FORBIDDEN otherwise.
 *
 * Fetches the required information from the database.
 * If the user id is given but the user cannot be found or if the kit cannot be found with the
 * given serial, the request is rejected with NOT_FOUND. If the request is *not* rejected, this
 * returns the fetched user, membership and kit.
 */
pub async fn fut_kit_permission_or_forbidden<'a>(
    pg: PgPool,
    user_id: Option<crate::models::UserId>,
    kit_serial: String,
    action: crate::authorization::KitAction,
) -> Result<
    (
        Option<crate::models::User>,
        Option<crate::models::KitMembership>,
        crate::models::Kit,
    ),
    Problem,
> {
    use diesel::Connection;

    let conn = pg.get().await?;
    threadpool(move || {
        conn.transaction(|| {
            let user = if let Some(user_id) = user_id {
                match crate::models::User::by_id(&conn, user_id)? {
                    Some(user) => Some(user),
                    // User id set but user is not found.
                    None => return Ok(None),
                }
            } else {
                None
            };

            let kit = match crate::models::Kit::by_serial(&conn, kit_serial)? {
                Some(kit) => kit,
                None => return Ok(None),
            };

            let membership = if let Some(user_id) = user_id {
                crate::models::KitMembership::by_user_id_and_kit_id(&conn, user_id, kit.get_id())?
            } else {
                None
            };

            Ok(Some((user, membership, kit)))
        })
    })
    .and_then(|v| async { some_or_not_found(v) })
    .and_then(|(user, membership, kit)| async move {
        // TODO clone should not be necessary.
        let kit_user = match (user.clone(), membership.clone()) {
            (None, _) => KitUser::Anonymous,
            (Some(user), None) => KitUser::User(user),
            (Some(user), Some(kit_membership)) => KitUser::UserWithMembership(user, kit_membership),
        };
        permission_or_forbidden(&kit_user, &kit, action).map(|_| (user, membership, kit))
    })
    .await
}

/**
 * Ensure the user has permission to perform the action on the target user.
 * Rejects the request with FORBIDDEN otherwise.
 *
 * Fetches the required information from the database.
 * If the actor user id is given but the user cannot be found or if the target user cannot be found with the
 * given username, the request is rejected with NOT_FOUND. If the request is *not* rejected, this
 * returns the fetched actor and target users.
 */
pub async fn fut_user_permission_or_forbidden(
    pg: PgPool,
    actor_user_id: Option<crate::models::UserId>,
    object_username: String,
    action: crate::authorization::UserAction,
) -> Result<(Option<crate::models::User>, crate::models::User), Problem> {
    use diesel::Connection;

    let conn = pg.get().await?;
    threadpool(move || {
        conn.transaction(|| {
            let actor_user = if let Some(actor_user_id) = actor_user_id {
                match crate::models::User::by_id(&conn, actor_user_id)? {
                    Some(user) => Some(user),
                    // User id set but user is not found.
                    None => return Ok(None),
                }
            } else {
                None
            };

            let object_user = match crate::models::User::by_username(&conn, &object_username)? {
                Some(user) => user,
                None => return Ok(None),
            };

            Ok(Some((actor_user, object_user)))
        })
    })
    .and_then(|v| async { some_or_not_found(v) })
    .and_then(|(target_user, object_user)| async move {
        permission_or_forbidden(&target_user, &object_user, action)
            .map(|_| (target_user, object_user))
    })
    .await
}

#[allow(dead_code)]
pub fn guard<T, E, F>(val: T, f: F) -> Result<T, E>
where
    F: Fn(&T) -> Option<E>,
{
    match f(&val) {
        Some(error) => Err(error),
        None => Ok(val),
    }
}
