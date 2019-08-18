use serde::Deserialize;
use warp::{filters::BoxedFilter, Filter, Rejection};

use crate::helpers;
use crate::models;
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
            |User {
                 username,
                 password,
                 email_address,
             },
             conn: crate::PgPooled| {
                trace!("Got request to create user with username: {}", username);
                let hash = astroplant_auth::hash::hash_user_password(&password);
                helpers::threadpool(move || {
                    let new_user = models::NewUser::new(&username, &hash, &email_address);
                    new_user.create(&conn)
                })
            },
        )
        .and_then(crate::helpers::ok_or_internal_error)
        .map(|res: Option<_>| Response::ok(res.is_some()))
}
