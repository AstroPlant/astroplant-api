use futures::future::{FutureExt, TryFutureExt};
use serde::Deserialize;
use validator::Validate;
use warp::{filters::BoxedFilter, path, Filter, Rejection};

use crate::response::{Response, ResponseBuilder};
use crate::{authentication, helpers, models, problem, views, PgPooled};

pub fn router(pg: BoxedFilter<(PgPooled,)>) -> BoxedFilter<(Response,)> {
    //impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    trace!("Setting up users router.");

    //TODO implement deleting users.
    (user_by_username(pg.clone()).boxed())
        .or(patch_user(pg.clone()).boxed())
        .unify()
        .or(list_kit_memberships(pg.clone()).boxed())
        .unify()
        .or(create_user(pg.clone()).boxed())
        .unify()
        .boxed()
}

// Handles the `GET /users/{username}` route.
pub fn user_by_username(
    pg: BoxedFilter<(PgPooled,)>,
) -> impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    warp::get()
        .and(path!(String))
        .and(authentication::option_by_token())
        .and(pg.clone())
        .and_then(
            |object_username: String, actor_user_id: Option<models::UserId>, conn: PgPooled| {
                helpers::fut_user_permission_or_forbidden(
                    conn,
                    actor_user_id,
                    object_username,
                    crate::authorization::UserAction::View,
                )
                .map_ok(|(_actor_user, object_user)| object_user)
            },
        )
        .map(move |user| ResponseBuilder::ok().body(views::User::from(user)))
}

// Handles the `PATCH /users/{username}` route.
pub fn patch_user(
    pg: BoxedFilter<(PgPooled,)>,
) -> impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    //TODO implement password patching.
    #[derive(Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    struct UserPatch {
        display_name: Option<String>,
        email_address: Option<String>,
        use_email_address_for_gravatar: Option<bool>,
    }

    warp::patch()
        .and(path!(String))
        .and(authentication::option_by_token())
        .and(pg.clone())
        .and_then(
            |object_username: String, actor_user_id: Option<models::UserId>, conn: PgPooled| {
                helpers::fut_user_permission_or_forbidden(
                    conn,
                    actor_user_id,
                    object_username,
                    crate::authorization::UserAction::EditDetails,
                )
                .map_ok(|(_actor_user, object_user)| object_user)
            },
        )
        .and(crate::helpers::deserialize())
        .and(pg)
        .and_then(
            move |user: models::User, user_patch: UserPatch, conn: PgPooled| async move {
                let update_user = models::UpdateUser {
                    id: user.id,
                    display_name: user_patch.display_name,
                    password_hash: None,
                    email_address: user_patch.email_address,
                    use_email_address_for_gravatar: user_patch.use_email_address_for_gravatar,
                };

                helpers::threadpool_diesel_ok(move || {
                    if let Some(email_address) = &update_user.email_address {
                        if let Some(user_by_email_address) =
                            models::User::by_email_address(&conn, email_address)?
                        {
                            if user_by_email_address.id != user.id {
                                let mut invalid_parameters = problem::InvalidParameters::new();
                                invalid_parameters.add(
                                    "emailAddress",
                                    problem::InvalidParameterReason::AlreadyExists,
                                );
                                return Ok(Err(warp::reject::custom(
                                    problem::Problem::InvalidParameters { invalid_parameters },
                                )));
                            }
                        }
                    }

                    if let Err(validation_errors) = update_user.validate() {
                        let invalid_parameters =
                            problem::InvalidParameters::from(validation_errors);
                        return Ok(Err(warp::reject::custom(
                            problem::Problem::InvalidParameters { invalid_parameters },
                        )));
                    }

                    let patched_user = update_user.update(&conn)?;
                    Ok(Ok(patched_user))
                })
                .map(helpers::flatten_result)
                .await
            },
        )
        .map(move |user| ResponseBuilder::ok().body(views::User::from(user)))
}

// Handles the `GET /users/{username}/kit-memberships` route.
pub fn list_kit_memberships(
    pg: BoxedFilter<(PgPooled,)>,
) -> impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    warp::get()
        .and(path!(String / "kit-memberships"))
        .and(authentication::option_by_token())
        .and(pg.clone())
        .and_then(
            |object_username: String, actor_user_id: Option<models::UserId>, conn: PgPooled| {
                helpers::fut_user_permission_or_forbidden(
                    conn,
                    actor_user_id,
                    object_username,
                    crate::authorization::UserAction::ListKitMemberships,
                )
                .map_ok(|(_actor_user, object_user)| object_user)
            },
        )
        .and(pg)
        .and_then(move |user: models::User, conn: PgPooled| async {
            let username = user.username.clone();
            let kit_memberships = helpers::threadpool_diesel_ok(move || {
                models::KitMembership::memberships_with_kit_of_user_id(&conn, user.get_id())
            })
            .await?;
            let v: Vec<views::KitMembership<String, views::Kit>> = kit_memberships
                .into_iter()
                .map(|(kit, membership)| {
                    views::KitMembership::from(membership)
                        .with_kit(views::Kit::from(kit))
                        .with_user(username.clone())
                })
                .collect();
            Ok::<_, Rejection>(ResponseBuilder::ok().body(v))
        })
}

pub fn create_user(
    pg: BoxedFilter<(PgPooled,)>,
) -> impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    use diesel::Connection;

    #[derive(Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    struct User {
        username: String,
        password: String,
        email_address: String,
    }

    warp::post()
        .and(warp::path::end())
        .and(crate::helpers::deserialize())
        .and(pg)
        .and_then(|user: User, conn: PgPooled| {
            let username = user.username.clone();
            trace!("Got request to create user with username: {}", username);

            helpers::threadpool_diesel_ok(move || {
                conn.transaction(|| {
                    let user_by_username = models::User::by_username(&conn, &user.username)?;
                    let user_by_email_address = models::User::by_email_address(&conn, &user.email_address)?;

                    let hash = astroplant_auth::hash::hash_user_password(&user.password);
                    let new_user = models::NewUser::new(user.username, hash, user.email_address);

                    if let Err(validation_errors) = new_user.validate() {
                        let invalid_parameters = problem::InvalidParameters::from(validation_errors);
                        return Ok(Err(warp::reject::custom(problem::Problem::InvalidParameters { invalid_parameters })))
                    }

                    let mut invalid_parameters = problem::InvalidParameters::new();
                    if user_by_username.is_some() {
                        invalid_parameters.add("username", problem::InvalidParameterReason::AlreadyExists)
                    }

                    if user_by_email_address.is_some() {
                        invalid_parameters.add("emailAddress", problem::InvalidParameterReason::AlreadyExists)
                    }

                    if !invalid_parameters.is_empty() {
                        return Ok(Err(warp::reject::custom(problem::Problem::InvalidParameters { invalid_parameters })))
                    }

                    let created_user = new_user.create(&conn)?;
                    if created_user.is_some() {
                        info!("Created user {:?}", username);

                        Ok(Ok(ResponseBuilder::created().empty()))
                    } else {
                        warn!("Unexpected database error: username and email address don't exist, yet user could not be created: {:?}", username);
                        Ok(Err(warp::reject::custom(problem::INTERNAL_SERVER_ERROR)))
                    }
                })
            }).map(helpers::flatten_result)
        })
}
