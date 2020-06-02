mod auth;

use futures::future::FutureExt;
use warp::{filters::BoxedFilter, path, Filter, Rejection};

use crate::database::PgPool;
use crate::problem::AppResult;
use crate::response::{Response, ResponseBuilder};
use crate::{authentication, helpers, models, views};

pub fn router(pg: PgPool) -> BoxedFilter<(AppResult<Response>,)> {
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

fn me(pg: PgPool) -> impl Filter<Extract = (AppResult<Response>,), Error = Rejection> + Clone {
    async fn implementation(pg: PgPool, user_id: models::UserId) -> AppResult<Response> {
        let conn = pg.get().await?;
        let user = helpers::threadpool_result(move || models::User::by_id(&conn, user_id)).await?;
        let user = helpers::some_or_internal_error(user)?;
        Ok(ResponseBuilder::ok().body(views::FullUser::from(user)))
    }

    authentication::by_token()
        .and_then(move |user_id: models::UserId| implementation(pg.clone(), user_id).never_error())
}
