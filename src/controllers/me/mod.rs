mod auth;

use warp::{filters::BoxedFilter, path, Filter, Rejection};

use crate::response::{Response, ResponseBuilder};
use crate::{authentication, helpers, models, problem, views};

pub fn router(pg: BoxedFilter<(crate::PgPooled,)>) -> BoxedFilter<(Response,)> {
    //impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    trace!("Setting up me router.");

    (path!("auth")
        .and(warp::path::end())
        .and(warp::post2())
        .and(auth::authenticate_by_credentials(pg.clone())))
    .or(path!("refresh")
        .and(warp::path::end())
        .and(warp::post2())
        .and(auth::authentication_token_from_refresh_token()))
    .unify()
    .or(warp::path::end().and(warp::get2()).and(me(pg.clone())))
    .unify()
    .or(path!("memberships")
        .and(warp::get2())
        .and(kit_memberships(pg.clone())))
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

/// Fetch kits belonging to the user.
fn kit_memberships(
    pg: BoxedFilter<(crate::PgPooled,)>,
) -> impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    authentication::by_token()
        .and(pg)
        .and_then(|user_id: models::UserId, conn: crate::PgPooled| {
            helpers::threadpool_diesel_ok(move || {
                models::KitMembership::memberships_with_kit_of_user_id(&conn, user_id)
            })
        })
        .map(
            |kit_memberships: Vec<(models::Kit, models::KitMembership)>| {
                let v: Vec<views::KitMembership<i32, views::Kit>> = kit_memberships
                    .into_iter()
                    .map(|(kit, membership)| {
                        views::KitMembership::from(membership).with_kit(views::Kit::from(kit))
                    })
                    .collect();
                ResponseBuilder::ok().body(v)
            },
        )
}
