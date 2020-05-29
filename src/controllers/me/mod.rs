mod auth;

use warp::{filters::BoxedFilter, path, Filter, Rejection};

use crate::response::{Response, ResponseBuilder};
use crate::{authentication, helpers, models, problem, views};

pub fn router(pg: BoxedFilter<(crate::PgPooled,)>) -> BoxedFilter<(Response,)> {
    //impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    trace!("Setting up me router.");

    (path!("auth")
        .and(warp::post())
        .and(auth::authenticate_by_credentials(pg.clone())))
    .or(path!("refresh")
        .and(warp::post())
        .and(auth::access_token_from_refresh_token()))
    .unify()
    .or(warp::path::end().and(warp::get()).and(me(pg.clone())))
    .unify()
    .boxed()
}

fn me(
    pg: BoxedFilter<(crate::PgPooled,)>,
) -> impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    authentication::by_token()
        .and(pg)
        .and_then(|user_id: models::UserId, conn: crate::PgPooled| {
            helpers::threadpool_diesel_ok(move || models::User::by_id(&conn, user_id))
        })
        .and_then(|user: Option<models::User>| {
            async {
                match user {
                    Some(user) => Ok(ResponseBuilder::ok().body(views::FullUser::from(user))),
                    None => Err(warp::reject::custom(problem::INTERNAL_SERVER_ERROR)),
                }
            }
        })
}
