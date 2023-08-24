mod auth;
pub use auth::{access_token_from_refresh_token, authenticate_by_credentials};

use axum::Extension;

use crate::database::PgPool;
use crate::problem::Problem;
use crate::response::{Response, ResponseBuilder};
use crate::{helpers, models, views};

pub async fn me(
    Extension(pg): Extension<PgPool>,
    user_id: crate::extract::UserId,
) -> Result<Response, Problem> {
    let mut conn = pg.get().await?;
    let user = helpers::threadpool_result(move || models::User::by_id(&mut conn, user_id)).await?;
    let user = helpers::some_or_internal_error(user)?;
    Ok(ResponseBuilder::ok().body(views::FullUser::from(user)))
}
