use axum::extract::Path;
use axum::Extension;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::database::PgPool;
use crate::problem::{Problem, NOT_FOUND};
use crate::response::{Response, ResponseBuilder};
use crate::{authorization, helpers, models, views};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Query {
    cursor: Option<String>,
    configuration: Option<i32>,
    peripheral: Option<i32>,
}

/// Handles the `GET /kits/{kitSerial}/media` route.
pub async fn kit_media(
    Extension(pg): Extension<PgPool>,
    user_id: Option<models::UserId>,
    Path(kit_serial): Path<String>,
    crate::extract::Query(query): crate::extract::Query<Query>,
) -> Result<Response, Problem> {
    use crate::cursors;
    use std::convert::TryFrom;

    let mut out_query = query.clone();
    let cursor = query.cursor.as_ref().map(|s| s.parse()).transpose()?;
    let base_uri = format!("/kits/{}/media", kit_serial);

    let (_user, _membership, kit) = helpers::fut_kit_permission_or_forbidden(
        pg.clone(),
        user_id,
        kit_serial,
        authorization::KitAction::View,
    )
    .await?;

    let conn = pg.get().await?;
    let mut response = ResponseBuilder::ok();
    let media = conn
        .interact_flatten_err(move |conn| {
            models::Media::page(
                conn,
                kit.get_id(),
                query.configuration,
                query.peripheral,
                cursor,
            )
        })
        .await?;

    if let Some(next_cursor) = cursors::Media::next_from_page(&media) {
        out_query.cursor = Some(next_cursor.into());
        let next_page_uri = format!(
            "{}?{}",
            base_uri,
            serde_urlencoded::to_string(&out_query).unwrap()
        );
        response = response.link(&next_page_uri, "next");
    }

    let body = media
        .into_iter()
        .map(views::Media::try_from)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(response.body(body))
}

/// Handles the `DELETE /media/{mediaId}` route.
pub async fn delete_media(
    Extension(pg): Extension<PgPool>,
    user_id: Option<models::UserId>,
    Path(media_id): Path<Uuid>,
) -> Result<Response, Problem> {
    use crate::schema::media;
    use diesel::prelude::*;
    let media_id = models::MediaId(media_id);

    // Check user authorization and make sure the configuration has never been activated.
    let conn = pg.clone().get().await?;

    {
        let kit = conn
            .interact_flatten_err(move |conn| {
                let kit = media::table
                    .inner_join(crate::schema::kits::table)
                    .filter(crate::schema::media::id.eq(media_id.id()))
                    .select(crate::models::Kit::as_select())
                    .first(conn)
                    .optional()?
                    .ok_or(NOT_FOUND)?;

                Ok::<_, Problem>(kit)
            })
            .await?;

        // FIXME: this unnecessarily queries for the kit: we already have it.
        helpers::fut_kit_permission_or_forbidden(
            pg,
            user_id,
            kit.serial.to_owned(),
            authorization::KitAction::DeleteMedia,
        )
        .await?;
    }

    conn.interact_flatten_err(move |conn| {
        use crate::schema::queue_media_pending_deletion;

        conn.transaction(|conn| {
            let selected_media = media::dsl::media.find(media_id.id());

            // 1. Add this media to the pending deletion queue.
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

            Ok::<_, diesel::result::Error>(())
        })?;

        Ok::<_, Problem>(ResponseBuilder::ok().empty())
    })
    .await
}

/// Handles the `GET /media/{mediaId}/content` route.
pub async fn download_media(
    Extension(pg): Extension<PgPool>,
    Extension(object_store): Extension<astroplant_object::ObjectStore>,
    user_id: Option<models::UserId>,
    Path(media_id): Path<Uuid>,
) -> Result<Response, Problem> {
    let media_id = models::MediaId(media_id);

    // Check user authorization and make sure the configuration has never been activated.
    let conn = pg.clone().get().await?;
    let (media, kit) = conn
        .interact_flatten_err(move |conn| {
            let media = models::Media::by_id(conn, media_id)?.ok_or(NOT_FOUND)?;
            let kit = models::Kit::by_id(conn, media.get_kit_id())?.ok_or(NOT_FOUND)?;

            Ok::<_, Problem>((media, kit))
        })
        .await?;

    // FIXME: this unnecessarily queries for the kit: we already have it.
    helpers::fut_kit_permission_or_forbidden(
        pg,
        user_id,
        kit.serial.to_owned(),
        authorization::KitAction::View,
    )
    .await?;

    let stream = object_store
        .get(&kit.serial, &media.id.hyphenated().to_string())
        .await
        .unwrap();

    Ok(ResponseBuilder::ok()
        .attachment_filename(&media.name)
        .stream(media.r#type, stream))
}
