use crate::problem::{Problem, FORBIDDEN, INTERNAL_SERVER_ERROR, NOT_FOUND};

use futures::future::TryFutureExt;
use log::error;
use serde::{de::DeserializeOwned, Deserialize};
use warp::{filters::BoxedFilter, Filter, Rejection};

use crate::{authentication, authorization, models};

/// Run a blocking function on a threadpool.
pub async fn threadpool<F, T>(f: F) -> T
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(f).await.unwrap()
}

/// Runs a function on a threadpool, ignoring a potential Diesel error inside the threadpool.
/// This error is turned into an internal server error (as Diesel errors are unexpected, and
/// indicative of erroneous queries).
pub async fn threadpool_diesel_ok<F, T>(f: F) -> Result<T, Rejection>
where
    F: FnOnce() -> Result<T, diesel::result::Error> + Send + 'static,
    T: Send + 'static,
{
    threadpool(f).await.map_err(|diesel_err| {
        error!("Error in diesel query: {:?}", diesel_err);
        warp::reject::custom(INTERNAL_SERVER_ERROR)
    })
}

/// Flatten a nested result with equal error types to a single result.
#[allow(dead_code)]
pub fn flatten_result<T, E>(nested: Result<Result<T, E>, E>) -> Result<T, E> {
    nested.and_then(|nested| nested)
}

/// Create a filter to deserialize a request.
pub fn deserialize<T>() -> impl Filter<Extract = (T,), Error = Rejection> + Copy
where
    T: DeserializeOwned + Send,
{
    // TODO: Also allow e.g. XML, basing the attempted deserialization on the Content-Type header.
    // Default to JSON.

    // Allow a request of at most 64 KiB
    const CONTENT_LENGTH_LIMIT: u64 = 1024 * 64;

    warp::body::content_length_limit(CONTENT_LENGTH_LIMIT)
        .or_else(|_| {
            futures::future::err(warp::reject::custom(Problem::PayloadTooLarge {
                limit: CONTENT_LENGTH_LIMIT,
            }))
        })
        .and(warp::body::bytes())
        .and_then(|body_buffer: bytes::Bytes| async {
            let body: Vec<u8> = body_buffer.into_iter().collect();

            serde_json::from_slice(&body).map_err(|err| {
                debug!("Request JSON deserialize error: {}", err);
                warp::reject::custom(Problem::InvalidJson {
                    category: (&err).into(),
                })
            })
        })
}

/// Create a filter to get a PostgreSQL connection from a PostgreSQL connection pool.
pub fn pg(
    pg_pool: crate::PgPool,
) -> impl Filter<Extract = (crate::PgPooled,), Error = Rejection> + Clone {
    warp::any()
        .map(move || pg_pool.clone())
        .and_then(|pg_pool: crate::PgPool| {
            // TODO: check whether PgPool::get actually needs to be run in a threadpool
            threadpool(move || match pg_pool.get() {
                Ok(pg_pooled) => Ok(pg_pooled),
                Err(_) => Err(warp::reject::custom(INTERNAL_SERVER_ERROR)),
            })
        })
}

#[allow(dead_code)]
pub fn ok_or_internal_error<T, E>(r: Result<T, E>) -> Result<T, Rejection> {
    match r {
        Ok(value) => Ok(value),
        Err(_) => Err(warp::reject::custom(INTERNAL_SERVER_ERROR)),
    }
}

#[allow(dead_code)]
pub fn some_or_internal_error<T>(r: Option<T>) -> Result<T, Rejection> {
    match r {
        Some(value) => Ok(value),
        None => Err(warp::reject::custom(INTERNAL_SERVER_ERROR)),
    }
}

#[allow(dead_code)]
pub fn some_or_not_found<T>(r: Option<T>) -> Result<T, Rejection> {
    match r {
        Some(value) => Ok(value),
        None => Err(warp::reject::custom(NOT_FOUND)),
    }
}

/**
 * Ensure the user has permission to perform the action on the kit.
 * Rejects the request otherwise.
 */
pub fn permission_or_forbidden(
    user: &Option<crate::models::User>,
    kit_membership: &Option<crate::models::KitMembership>,
    kit: &crate::models::Kit,
    action: crate::authorization::KitAction,
) -> Result<(), Rejection> {
    if action.permission(user, kit_membership, kit) {
        Ok(())
    } else {
        Err(warp::reject::custom(FORBIDDEN))
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
pub async fn fut_permission_or_forbidden<'a>(
    conn: crate::PgPooled,
    user_id: Option<crate::models::UserId>,
    kit_serial: String,
    action: crate::authorization::KitAction,
) -> Result<
    (
        Option<crate::models::User>,
        Option<crate::models::KitMembership>,
        crate::models::Kit,
    ),
    Rejection,
> {
    use diesel::Connection;

    threadpool_diesel_ok(move || {
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
        permission_or_forbidden(&user, &membership, &kit, action).map(|_| (user, membership, kit))
    })
    .await
}

/**
 * Authenticate the user through the Authorization header and the kit from the kitSerial parameter
 * in the query. Check whether the user is authorized to perform the given action. Returns the
 * user, kit membership and kit fetched from the database.
 */
pub fn authorization_user_kit_from_query(
    pg: BoxedFilter<(crate::PgPooled,)>,
    action: authorization::KitAction,
) -> BoxedFilter<(
    Option<models::User>,
    Option<models::KitMembership>,
    models::Kit,
)> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct KitSerial {
        kit_serial: String,
    }

    warp::query::query::<KitSerial>()
        .map(|query: KitSerial| query.kit_serial)
        .and(authentication::option_by_token())
        .and(pg)
        .and_then(
            move |kit_serial: String, user_id: Option<models::UserId>, conn: crate::PgPooled| {
                fut_permission_or_forbidden(conn, user_id, kit_serial, action)
            },
        )
        .untuple_one()
        .boxed()
}

pub fn guard<T, F>(val: T, f: F) -> Result<T, warp::Rejection>
where
    F: Fn(&T) -> Option<warp::Rejection>,
{
    match f(&val) {
        Some(rejection) => Err(rejection),
        None => Ok(val),
    }
}

#[cfg(test)]
mod test {
    use crate::problem::{JsonDeserializeErrorCategory, Problem};
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct TestStruct {
        value: String,
    }

    #[test]
    fn flatten() {
        assert_eq!(
            super::flatten_result::<_, std::convert::Infallible>(Ok(Ok(42))),
            Ok(42)
        );
        assert_eq!(
            super::flatten_result::<std::convert::Infallible, _>(Ok(Err("oops"))),
            Err("oops")
        );
        assert_eq!(
            super::flatten_result::<std::convert::Infallible, _>(Err("oops")),
            Err("oops")
        );
    }

    #[test]
    fn deserialize_json() {
        futures::executor::block_on(async {
            let value: TestStruct = warp::test::request()
                .header("Accept", "application/json")
                .body(r#"{"value":"It all adds up to normality."}"#)
                .filter(&super::deserialize())
                .await
                .unwrap();
            assert_eq!(value.value, "It all adds up to normality.");
        })
    }

    #[test]
    fn reject_content_length_limit() {
        futures::executor::block_on(async {
            // Construct a large request.
            let body = format!(
                "{}{}{}",
                r#"{"value":""#,
                vec!['.'; 1024 * 64 - 12 + 1]
                    .into_iter()
                    .collect::<String>(),
                r#""}"#
            );
            // Should reject requests with too large Content-Length.
            let response = warp::test::request()
                .header("Accept", "application/json")
                .body(body)
                .filter(&super::deserialize::<TestStruct>())
                .await;
            assert!(match response {
                Err(rejection) => {
                    match rejection.find::<Problem>() {
                        Some(Problem::PayloadTooLarge { .. }) => true,
                        _ => false,
                    }
                }
                _ => false,
            });
        })
    }

    #[test]
    fn reject_syntactically_incorrect_json() {
        futures::executor::block_on(async {
            let response = warp::test::request()
                .header("Accept", "application/json")
                .body(r#"{"value"."It does not add up to normality."}"#)
                .filter(&super::deserialize::<TestStruct>())
                .await;
            assert!(match response {
                Err(rejection) => {
                    match rejection.find::<Problem>() {
                        Some(Problem::InvalidJson {
                            category: JsonDeserializeErrorCategory::Syntactic,
                        }) => true,
                        _ => false,
                    }
                }
                _ => false,
            });

            let response = warp::test::request()
                .header("Accept", "application/json")
                .body(r#"{"value":"It does not add up to normality.}"#)
                .filter(&super::deserialize::<TestStruct>())
                .await;
            assert!(match response {
                Err(rejection) => {
                    match rejection.find::<Problem>() {
                        Some(Problem::InvalidJson {
                            category: JsonDeserializeErrorCategory::PrematureEnd,
                        }) => true,
                        _ => false,
                    }
                }
                _ => false,
            });
        })
    }

    #[test]
    fn reject_semantically_incorrect_json() {
        futures::executor::block_on(async {
            let response = warp::test::request()
                .header("Accept", "application/json")
                .body(r#"{"value":42}"#)
                .filter(&super::deserialize::<TestStruct>())
                .await;
            assert!(match response {
                Err(rejection) => {
                    match rejection.find::<Problem>() {
                        Some(Problem::InvalidJson {
                            category: JsonDeserializeErrorCategory::Semantic,
                        }) => true,
                        _ => false,
                    }
                }
                _ => false,
            });
        })
    }
}
