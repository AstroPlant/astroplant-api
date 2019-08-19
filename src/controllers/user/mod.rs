use futures::future::Future;
use serde::Deserialize;
use warp::{filters::BoxedFilter, Filter, Rejection};

use crate::helpers;
use crate::models;
use crate::problem;
use crate::response::Response;

pub fn router(
    pg: BoxedFilter<(crate::PgPooled,)>,
) -> impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    trace!("Setting up users router.");

    warp::path::end()
        .and(warp::post2())
        .and(create_user(pg.clone()))
}

pub fn create_user(
    pg: BoxedFilter<(crate::PgPooled,)>,
) -> impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    use diesel::Connection;

    #[derive(Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    struct User {
        username: String,
        password: String,
        email_address: String, // todo: validate email matches regex .+@.+\..+
    }

    crate::helpers::json_decode()
        .and(pg)
        .and_then(
            |user: User, conn: crate::PgPooled| {
                trace!("Got request to create user with username: {}", user.username);

                helpers::threadpool_diesel_ok(move || {
                    conn.transaction(|| {
                        let user_by_username = models::User::by_username(&conn, &user.username)?;
                        let user_by_email_address = models::User::by_email_address(&conn, &user.email_address)?;

                        let mut errors = vec![];
                        if user_by_username.is_some() {
                            errors.push(problem::InvalidParameter::already_exists("username".to_owned()))
                        }

                        if user_by_email_address.is_some() {
                            errors.push(problem::InvalidParameter::already_exists("emailAddress".to_owned()))
                        }

                        if !errors.is_empty() {
                            return Ok(Err(warp::reject::custom(problem::Problem::InvalidParameters { invalid_parameters: errors })))
                        }

                        let hash = astroplant_auth::hash::hash_user_password(&user.password);
                        let new_user = models::NewUser::new(&user.username, &hash, &user.email_address);
                        let created_user = new_user.create(&conn)?;

                        if created_user.is_some() {
                            info!("Created user {:?}", user);

                            Ok(Ok(Response::created_empty()))
                        } else {
                            warn!("Unexpected database error: username and email address don't exist, yet user could not be created: {:?}", user);
                            Ok(Err(warp::reject::custom(problem::INTERNAL_SERVER_ERROR)))
                        }
                    })
                }).then(helpers::flatten_result)
            },
        )
}
