use axum::extract::Path;
use axum::Extension;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::authorization::KitAction;
use crate::database::PgPool;
use crate::helpers::kit_permission_or_forbidden;
use crate::models::{KitMembership, User};
use crate::problem::{self, Problem};
use crate::response::{Response, ResponseBuilder};
use crate::utils::deserialize_some;
use crate::{helpers, models, schema, views};

mod archive;
pub use archive::{archive, archive_authorize};

#[derive(Deserialize)]
pub struct CursorPage {
    after: Option<i32>,
}

/// Handles the `GET /kits/?after=afterId` route.
pub async fn kits(
    Extension(pg): Extension<PgPool>,
    cursor: crate::extract::Query<CursorPage>,
) -> Result<Response, Problem> {
    let conn = pg.get().await?;
    let kits = conn
        .interact_flatten_err(move |conn| {
            models::Kit::cursor_page(conn, cursor.after, 100)
                .map(|kits| kits.into_iter().map(views::Kit::from).collect::<Vec<_>>())
        })
        .await?;

    let next_page_uri = kits.last().map(|last| format!("/kits?after={}", last.id));

    let mut response_builder = ResponseBuilder::ok();
    if let Some(next_page_uri) = next_page_uri {
        response_builder = response_builder.next_page_uri(&next_page_uri);
    }
    Ok(response_builder.body(kits))
}

pub async fn kit_by_serial(
    Extension(pg): Extension<PgPool>,
    Path(kit_serial): Path<String>,
    user_id: Option<crate::extract::UserId>,
) -> Result<Response, Problem> {
    let (_, _, kit) = helpers::fut_kit_permission_or_forbidden(
        pg,
        user_id,
        kit_serial,
        crate::authorization::KitAction::View,
    )
    .await?;
    Ok(ResponseBuilder::ok().body(views::Kit::from(kit)))
}

/// Handles the `POST /kits/{kitSerial}/password` route.
pub async fn reset_password(
    Extension(pg): Extension<PgPool>,
    Path(kit_serial): Path<String>,
    user_id: Option<crate::extract::UserId>,
) -> Result<Response, Problem> {
    let (_, _, kit) = helpers::fut_kit_permission_or_forbidden(
        pg.clone(),
        user_id,
        kit_serial,
        crate::authorization::KitAction::ResetPassword,
    )
    .await?;
    let conn = pg.get().await?;
    let password = conn
        .interact_flatten_err(move |conn| {
            let (update_kit, password) =
                models::UpdateKit::unchanged_for_id(kit.id).reset_password();
            update_kit.update(conn)?;
            Ok::<_, Problem>(password)
        })
        .await?;
    Ok(ResponseBuilder::ok().body(password))
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CreateKit {
    name: Option<String>,
    description: Option<String>,
    latitude: Option<f64>,
    longitude: Option<f64>,
    privacy_public_dashboard: bool,
    privacy_show_on_map: bool,
}

/// Handles the `POST /kits` route.
pub async fn create_kit(
    Extension(pg): Extension<PgPool>,
    user_id: crate::extract::UserId,
    crate::extract::Json(kit): crate::extract::Json<CreateKit>,
) -> Result<Response, Problem> {
    use bigdecimal::{BigDecimal, FromPrimitive};
    use diesel::Connection;
    use validator::Validate;

    #[derive(Serialize, Debug)]
    #[serde(rename_all = "camelCase")]
    struct Created {
        kit_serial: String,
        password: String,
    }

    let (new_kit, password) = models::NewKit::new_with_generated_password(
        kit.name,
        kit.description,
        kit.latitude.and_then(BigDecimal::from_f64),
        kit.longitude.and_then(BigDecimal::from_f64),
        kit.privacy_public_dashboard,
        kit.privacy_show_on_map,
    );

    if let Err(validation_errors) = new_kit.validate() {
        let invalid_parameters = problem::InvalidParameters::from(validation_errors);
        return Err(problem::Problem::InvalidParameters { invalid_parameters });
    };

    let conn = pg.get().await?;
    conn.interact(move |conn| {
        conn.transaction(|conn| {
            let created_kit: models::Kit = new_kit.create(conn)?;
            let kit_serial = created_kit.serial;
            tracing::debug!("Created kit \"{}\"", kit_serial);
            let kit_id = models::KitId(created_kit.id);

            models::NewKitMembership::new(user_id, kit_id, true, true).create(conn)?;

            let response = ResponseBuilder::created().body(Created {
                kit_serial,
                password,
            });

            Ok(response)
        })
    })
    .await?
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct KitPatch {
    #[serde(default, deserialize_with = "deserialize_some")]
    name: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_some")]
    description: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_some")]
    latitude: Option<Option<f64>>,
    #[serde(default, deserialize_with = "deserialize_some")]
    longitude: Option<Option<f64>>,
    privacy_public_dashboard: Option<bool>,
    privacy_show_on_map: Option<bool>,
}

/// Handles the `PATCH /kits/{kitSerial}` route.
pub async fn patch_kit(
    Extension(pg): Extension<PgPool>,
    Path(kit_serial): Path<String>,
    user_id: Option<crate::extract::UserId>,
    crate::extract::Json(kit_patch): crate::extract::Json<KitPatch>,
) -> Result<Response, Problem> {
    use bigdecimal::{BigDecimal, FromPrimitive};

    let (_, _, kit) = helpers::fut_kit_permission_or_forbidden(
        pg.clone(),
        user_id,
        kit_serial,
        crate::authorization::KitAction::EditDetails,
    )
    .await?;

    let update_kit = models::UpdateKit {
        id: kit.id,
        name: kit_patch.name,
        description: kit_patch.description,
        latitude: kit_patch.latitude.map(|l| l.and_then(BigDecimal::from_f64)),
        longitude: kit_patch
            .longitude
            .map(|l| l.and_then(BigDecimal::from_f64)),
        privacy_public_dashboard: kit_patch.privacy_public_dashboard,
        privacy_show_on_map: kit_patch.privacy_show_on_map,
        password_hash: None,
    };

    let conn = pg.get().await?;
    conn.interact(move |conn| {
        let patched_kit = update_kit.update(conn)?;
        Ok(ResponseBuilder::ok().body(views::Kit::from(patched_kit)))
    })
    .await?
}

/// Handles the `DELETE /kits/{kitSerial}` route.
///
/// All configurations, peripherals, raw measurements, and aggregate measurements belonging to this
/// kit are deleted. Media belonging to this kit are orphaned and placed in the
/// media-pending-deletion queue.
pub async fn delete_kit(
    Extension(pg): Extension<PgPool>,
    Path(kit_serial): Path<String>,
    user_id: Option<models::UserId>,
) -> Result<Response, Problem> {
    let (_, _, kit) = helpers::fut_kit_permission_or_forbidden(
        pg.clone(),
        user_id,
        kit_serial,
        crate::authorization::KitAction::Delete,
    )
    .await?;

    let conn = pg.get().await?;
    conn.interact_flatten_err(move |conn| {
        use diesel::prelude::*;
        use schema::media;
        use schema::queue_media_pending_deletion;

        conn.transaction(|conn| {
            let selected_media = media::dsl::media.filter(media::kit_id.eq(kit.id));

            // 1. Move media belonging to this kit to the pending deletion queue.
            selected_media
                .select((media::id, media::datetime, media::size))
                .insert_into(queue_media_pending_deletion::table)
                .into_columns((
                    queue_media_pending_deletion::media_id,
                    queue_media_pending_deletion::media_datetime,
                    queue_media_pending_deletion::media_size,
                ))
                .execute(conn)?;

            // 2. Delete this media from the media table.
            diesel::delete(selected_media).execute(conn)?;

            // 3. And finally delete the kit itself.
            diesel::delete(&kit).execute(conn)?;

            Ok::<_, diesel::result::Error>(())
        })?;

        Ok::<_, Problem>(ResponseBuilder::ok().empty())
    })
    .await
}

/// Handles the `GET /kits/{kitSerial}/members` route.
pub async fn get_members(
    Extension(pg): Extension<PgPool>,
    Path(kit_serial): Path<String>,
    user_id: Option<models::UserId>,
) -> Result<Response, Problem> {
    let (_, _, kit) = helpers::fut_kit_permission_or_forbidden(
        pg.clone(),
        user_id,
        kit_serial,
        crate::authorization::KitAction::View,
    )
    .await?;

    let conn = pg.get().await?;

    let kit_id = kit.id;
    let members: Vec<(User, KitMembership)> = conn
        .interact_flatten_err(move |conn| {
            use diesel::prelude::*;
            use schema::kit_memberships;
            use schema::users;

            users::table
                .inner_join(kit_memberships::table)
                .filter(kit_memberships::kit_id.eq(kit_id))
                .get_results(conn)
        })
        .await?;

    let v: Vec<_> = members
        .into_iter()
        .map(|(user, membership)| {
            views::KitMembership::from(membership)
                .with_kit(views::Kit::from(kit.clone()))
                .with_user(views::User::from(user))
        })
        .collect();

    Ok(ResponseBuilder::ok().body(v))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddMember {
    username: String,
    access_configure: bool,
    access_super: bool,
}

/// Handles the `POST /kits/{kitSerial}/members` route.
pub async fn add_member(
    Extension(pg): Extension<PgPool>,
    Path(kit_serial): Path<String>,
    user_id: Option<models::UserId>,
    crate::extract::Json(member): crate::extract::Json<AddMember>,
) -> Result<Response, Problem> {
    let action = if member.access_super {
        crate::authorization::KitAction::EditSuperMembers
    } else {
        crate::authorization::KitAction::EditMembers
    };
    let (_, _, kit) =
        helpers::fut_kit_permission_or_forbidden(pg.clone(), user_id, kit_serial, action).await?;

    let conn = pg.get().await?;

    let kit_id = kit.id;
    let membership = conn
        .interact_flatten_err(move |conn| {
            use diesel::prelude::*;
            use schema::kit_memberships;
            use schema::users;

            conn.build_transaction().serializable().run(move |conn| {
                let user: User = users::table
                    .filter(users::username.eq(&member.username))
                    .first(conn)?;

                let existing_membership: Option<KitMembership> = kit_memberships::table
                    .filter(kit_memberships::kit_id.eq(kit_id))
                    .filter(kit_memberships::user_id.eq(user.id))
                    .get_result(conn)
                    .optional()?;

                if let Some(existing_membership) = existing_membership {
                    // This membership already exists, do nothing
                    return Ok::<_, Problem>(
                        views::KitMembership::from(existing_membership)
                            .with_user(views::User::from(user))
                            .with_kit(views::Kit::from(kit)),
                    );
                }

                #[derive(Insertable)]
                #[diesel(table_name = kit_memberships)]
                struct NewKitMembership {
                    user_id: i32,
                    kit_id: i32,
                    access_super: bool,
                    access_configure: bool,
                    datetime_linked: DateTime<Utc>,
                }

                let membership = NewKitMembership {
                    user_id: user.id,
                    kit_id,
                    access_super: member.access_super,
                    access_configure: member.access_configure,
                    datetime_linked: Utc::now(),
                };
                let membership: KitMembership = membership
                    .insert_into(kit_memberships::table)
                    .get_result(conn)?;

                Ok::<_, Problem>(
                    views::KitMembership::from(membership)
                        .with_user(views::User::from(user))
                        .with_kit(views::Kit::from(kit)),
                )
            })
        })
        .await?;

    Ok(ResponseBuilder::ok().body(membership))
}

#[derive(Deserialize)]
pub struct MemberSuggestions {
    username: String,
}

/// Handles the `GET /kits/{kitSerial}/member-suggestions` route.
pub async fn get_member_suggestions(
    Extension(pg): Extension<PgPool>,
    Path(kit_serial): Path<String>,
    user_id: Option<models::UserId>,
    query: crate::extract::Query<MemberSuggestions>,
) -> Result<Response, Problem> {
    // TODO: make suggestions smarter, e.g., by showing users higher in the results that are
    // members of the querying user's other kits or members of kits of the current kit's members.

    use diesel::prelude::*;
    use schema::users;

    let (_, _, _) = helpers::fut_kit_permission_or_forbidden(
        pg.clone(),
        user_id,
        kit_serial,
        crate::authorization::KitAction::EditMembers,
    )
    .await?;

    let conn = pg.get().await?;

    let users: Vec<User> = conn
        .interact_flatten_err(move |conn| {
            users::table
                .filter(users::username.ilike(format!("%{}%", &query.username)))
                .limit(20)
                .get_results(conn)
        })
        .await?;
    let users: Vec<views::User> = users
        .into_iter()
        .map(|user| views::User::from(user))
        .collect();

    Ok(ResponseBuilder::ok().body(users))
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct KitMembershipPatch {
    access_configure: bool,
    access_super: bool,
}

/// Handles the `PATCH /kit-memberships/{id}` route.
pub async fn patch_kit_membership(
    Extension(pg): Extension<PgPool>,
    Path(kit_membership_id): Path<i32>,
    user_id: Option<models::UserId>,
    crate::extract::Json(kit_membership_patch): crate::extract::Json<KitMembershipPatch>,
) -> Result<Response, Problem> {
    let conn = pg.get().await?;

    let updated_kit_membership = conn
        .interact_flatten_err(move |conn| {
            use diesel::prelude::*;
            use schema::kit_memberships;
            use schema::kits;

            conn.build_transaction().serializable().run(move |conn| {
                let kit_membership: KitMembership =
                    kit_memberships::table.find(kit_membership_id).first(conn)?;
                let kit = kits::table.find(kit_membership.kit_id).first(conn)?;

                {
                    let permission_action =
                        if kit_membership.access_super != kit_membership_patch.access_super {
                            KitAction::EditSuperMembers
                        } else {
                            KitAction::EditMembers
                        };
                    kit_permission_or_forbidden(conn, user_id, &kit, permission_action)?;
                }

                if kit_membership.access_super && !kit_membership_patch.access_super {
                    // removing super access of a member, there must be at least one other member
                    // with super access to allow this
                    let members_with_super_access: i64 = kit_memberships::table
                        .filter(kit_memberships::kit_id.eq(kit.id))
                        .filter(kit_memberships::access_super.eq(true))
                        .count()
                        .get_result(conn)?;

                    if members_with_super_access < 2 {
                        return Err(Problem::KitsRequireOneSuperMember);
                    }
                }

                let membership = {
                    #[derive(Identifiable, AsChangeset)]
                    #[diesel(
                    table_name = kit_memberships,
                )]
                    pub struct UpdateKitMembership {
                        id: i32,
                        access_super: bool,
                        access_configure: bool,
                    }
                    let update = UpdateKitMembership {
                        id: kit_membership.id,
                        access_super: kit_membership_patch.access_super,
                        access_configure: kit_membership_patch.access_configure,
                    };
                    let membership: KitMembership = update.save_changes(conn)?;
                    membership
                };

                Ok::<_, Problem>(membership)
            })
        })
        .await?;

    Ok(ResponseBuilder::ok().body(views::KitMembership::from(updated_kit_membership)))
}

/// Handles the `DELETE /kit-memberships/{id}` route.
pub async fn delete_kit_membership(
    Extension(pg): Extension<PgPool>,
    Path(kit_membership_id): Path<i32>,
    user_id: Option<models::UserId>,
) -> Result<Response, Problem> {
    let conn = pg.get().await?;

    conn.interact_flatten_err(move |conn| {
        use diesel::prelude::*;
        use schema::kit_memberships;
        use schema::kits;

        conn.build_transaction().serializable().run(move |conn| {
            let kit_membership: KitMembership =
                kit_memberships::table.find(kit_membership_id).first(conn)?;
            let kit = kits::table.find(kit_membership.kit_id).first(conn)?;

            {
                let permission_action = if kit_membership.access_super {
                    KitAction::EditSuperMembers
                } else {
                    KitAction::EditMembers
                };
                kit_permission_or_forbidden(conn, user_id, &kit, permission_action)?;
            }

            if kit_membership.access_super {
                // deleting a member with super access, there must be at least one other member
                // with super access to allow this
                let members_with_super_access: i64 = kit_memberships::table
                    .filter(kit_memberships::kit_id.eq(kit.id))
                    .filter(kit_memberships::access_super.eq(true))
                    .count()
                    .get_result(conn)?;

                if members_with_super_access < 2 {
                    return Err(Problem::KitsRequireOneSuperMember);
                }
            }

            diesel::delete(&kit_membership).execute(conn)?;

            Ok::<_, Problem>(())
        })
    })
    .await?;

    Ok(ResponseBuilder::ok().empty())
}
