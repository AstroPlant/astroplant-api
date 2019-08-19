mod auth;

use warp::{filters::BoxedFilter, path, Filter, Rejection};

use crate::authentication::authenticate_by_token;
use crate::helpers;
use crate::models;
use crate::problem;
use crate::response::Response;
use crate::views;

pub fn router(
    pg: BoxedFilter<(crate::PgPooled,)>,
) -> impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    trace!("Setting up me router.");

    (path!("auth")
        .and(warp::path::end())
        .and(warp::post2())
        .and(auth::authenticate_by_credentials(pg.clone())))
    .or(path!("refresh")
        .and(warp::path::end())
        .and(warp::post2())
        .and(auth::normal_token_from_refresh_token()))
    .unify()
    .or(warp::path::end().and(warp::get2()).and(me(pg.clone())))
    .unify()
}

pub fn me(
    pg: BoxedFilter<(crate::PgPooled,)>,
) -> impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    authenticate_by_token()
        .and(pg)
        .and_then(|user_id: models::UserId, conn: crate::PgPooled| {
            helpers::threadpool_diesel_ok(move || models::User::by_id(&conn, user_id))
        })
        .and_then(|user: Option<models::User>| match user {
            Some(user) => Ok(Response::ok(views::FullUser::from(user))),
            None => Err(warp::reject::custom(problem::INTERNAL_SERVER_ERROR)),
        })
}
