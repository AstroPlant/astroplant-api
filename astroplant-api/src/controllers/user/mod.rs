use axum::extract::Path;
use axum::Extension;
use diesel::prelude::*;
use serde::Deserialize;
use validator::Validate;

use crate::database::PgPool;
use crate::models::{Kit, KitMembership};
use crate::problem::{self, Problem};
use crate::response::{Response, ResponseBuilder};
use crate::schema::{kit_last_seen, kits};
use crate::{helpers, models, views};

// Handles the `GET /users/{username}` route.
pub async fn user_by_username(
    Extension(pg): Extension<PgPool>,
    Path(object_username): Path<String>,
    actor_user_id: Option<models::UserId>,
) -> Result<Response, Problem> {
    let (_target_user, object_user) = helpers::fut_user_permission_or_forbidden(
        pg,
        actor_user_id,
        object_username,
        crate::authorization::UserAction::View,
    )
    .await?;

    Ok(ResponseBuilder::ok().body(views::User::from(object_user)))
}

// TODO implement password patching.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UserPatch {
    display_name: Option<String>,
    email_address: Option<String>,
    use_email_address_for_gravatar: Option<bool>,
}

// Handles the `PATCH /users/{username}` route.
pub async fn patch_user(
    Extension(pg): Extension<PgPool>,
    Path(object_username): Path<String>,
    actor_user_id: Option<models::UserId>,
    crate::extract::Json(user_patch): crate::extract::Json<UserPatch>,
) -> Result<Response, Problem> {
    let (_actor_user, user) = helpers::fut_user_permission_or_forbidden(
        pg.clone(),
        actor_user_id,
        object_username,
        crate::authorization::UserAction::EditDetails,
    )
    .await?;

    let update_user = models::UpdateUser {
        id: user.id,
        display_name: user_patch.display_name,
        password_hash: None,
        email_address: user_patch.email_address,
        use_email_address_for_gravatar: user_patch.use_email_address_for_gravatar,
    };

    let conn = pg.get().await?;
    let patched_user = conn
        .interact_flatten_err(move |conn| {
            if let Some(email_address) = &update_user.email_address {
                if let Some(user_by_email_address) =
                    models::User::by_email_address(conn, email_address)?
                {
                    if user_by_email_address.id != user.id {
                        let mut invalid_parameters = problem::InvalidParameters::new();
                        invalid_parameters.add(
                            "emailAddress",
                            problem::InvalidParameterReason::AlreadyExists,
                        );
                        return Err(problem::Problem::InvalidParameters { invalid_parameters });
                    }
                }
            }

            if let Err(validation_errors) = update_user.validate() {
                let invalid_parameters = problem::InvalidParameters::from(validation_errors);
                return Err(problem::Problem::InvalidParameters { invalid_parameters });
            }

            Ok::<_, Problem>(update_user.update(conn)?)
        })
        .await?;

    Ok(ResponseBuilder::ok().body(views::User::from(patched_user)))
}

// Handles the `GET /users/{username}/kit-memberships` route.
pub async fn list_kit_memberships(
    Extension(pg): Extension<PgPool>,
    Path(object_username): Path<String>,
    actor_user_id: Option<models::UserId>,
) -> Result<Response, Problem> {
    let (_actor_user, user) = helpers::fut_user_permission_or_forbidden(
        pg.clone(),
        actor_user_id,
        object_username,
        crate::authorization::UserAction::ListKitMemberships,
    )
    .await?;

    // TODO: perhaps only return kits that the querying user (not the user being queried) is
    // allowed to view (i.e., those kits that have a public dashboard, or those that the querying
    // user is a member of)

    let user_id = user.get_id();
    let conn = pg.get().await?;
    let kit_memberships = conn
        .interact(move |conn| {
            KitMembership::by_user_id(user_id)
                .inner_join(kits::table.left_join(kit_last_seen::table))
                .select((
                    KitMembership::as_select(),
                    Kit::as_select(),
                    kit_last_seen::datetime_last_seen.nullable(),
                ))
                .get_results(conn)
        })
        .await??;

    let v: Vec<views::KitMembership<views::User, views::Kit>> = kit_memberships
        .into_iter()
        .map(|(membership, kit, kit_last_seen)| {
            views::KitMembership::from(membership)
                .with_kit(views::Kit::from((kit, kit_last_seen)))
                .with_user(views::User::from(user.clone()))
        })
        .collect();
    Ok(ResponseBuilder::ok().body(v))
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct User {
    username: String,
    password: String,
    email_address: String,
}

pub async fn create_user(
    Extension(pg): Extension<PgPool>,
    crate::extract::Json(user): crate::extract::Json<User>,
) -> Result<Response, Problem> {
    let username = user.username.clone();
    tracing::trace!("Got request to create user with username: {}", username);

    let conn = pg.get().await?;
    conn.interact_flatten_err(move |conn| {
        conn.transaction(|conn| {
            let user_by_username = models::User::by_username(conn, &user.username)?;
            let user_by_email_address = models::User::by_email_address(conn, &user.email_address)?;

            let hash = astroplant_auth::hash::hash_user_password(&user.password);
            let new_user = models::NewUser::new(user.username, hash, user.email_address);

            if let Err(validation_errors) = new_user.validate() {
                let invalid_parameters = problem::InvalidParameters::from(validation_errors);
                return Err(problem::Problem::InvalidParameters { invalid_parameters })
            }

            let mut invalid_parameters = problem::InvalidParameters::new();
            if user_by_username.is_some() {
                invalid_parameters.add("username", problem::InvalidParameterReason::AlreadyExists)
            }

            if user_by_email_address.is_some() {
                invalid_parameters.add("emailAddress", problem::InvalidParameterReason::AlreadyExists)
            }

            if !invalid_parameters.is_empty() {
                return Err(problem::Problem::InvalidParameters { invalid_parameters })
            }

            let created_user = new_user.create(conn)?;
            if created_user.is_some() {
                tracing::info!("Created user {:?}", username);

                Ok(ResponseBuilder::created().empty())
            } else {
                tracing::warn!("Unexpected database error: username and email address don't exist, yet user could not be created: {:?}", username);
                Err(problem::INTERNAL_SERVER_ERROR)
            }
        })
    }).await
}
