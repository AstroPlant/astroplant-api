use futures::future::FutureExt;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use warp::{filters::BoxedFilter, Filter, Rejection};

use crate::database::PgPool;
use crate::problem::{AppResult, Problem, NOT_FOUND};
use crate::response::{Response, ResponseBuilder};
use crate::{authentication, authorization, helpers, models, views};

pub fn router(
    pg: PgPool,
    object_store: astroplant_object::ObjectStore,
) -> BoxedFilter<(AppResult<Response>,)> {
    //impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    trace!("Setting up media router.");

    kit_media(pg.clone())
        .or(download_media(pg.clone(), object_store))
        .unify()
        .boxed()
}

/// Handles the `GET /kits/{kitSerial}/media` route.
fn kit_media(
    pg: PgPool,
) -> impl Filter<Extract = (AppResult<Response>,), Error = Rejection> + Clone {
    #[derive(Clone, Debug, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Query {
        cursor: Option<String>,
        configuration: Option<i32>,
        peripheral: Option<i32>,
    }

    async fn implementation(
        pg: PgPool,
        kit_serial: String,
        user_id: Option<models::UserId>,
        query: Query,
    ) -> AppResult<Response> {
        use crate::cursors;
        use std::convert::TryFrom;

        let mut out_query = query.clone();
        let cursor = (&query).cursor.as_ref().map(|s| s.parse()).transpose()?;
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
        let media = helpers::threadpool(move || {
            models::Media::page(
                &conn,
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
            .map(|media| views::Media::try_from(media))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(response.body(body))
    }

    warp::get()
        .and(warp::path!("kits" / String / "media"))
        .and(authentication::option_by_token())
        .and(warp::query())
        .and_then(move |kit_serial, user_id, query: Query| {
            implementation(pg.clone(), kit_serial, user_id, query).never_error()
        })
}

/// Handles the `GET` /media/{mediaId}/content` route.
fn download_media(
    pg: PgPool,
    object_store: astroplant_object::ObjectStore,
) -> impl Filter<Extract = (AppResult<Response>,), Error = Rejection> + Clone {
    /// Check user authorization and make sure the configuration has never been activated.
    async fn implementation(
        pg: PgPool,
        object_store: astroplant_object::ObjectStore,
        user_id: Option<models::UserId>,
        media_id: models::MediaId,
    ) -> AppResult<Response> {
        let conn = pg.clone().get().await?;
        let (media, kit) = helpers::threadpool(move || {
            let media = models::Media::by_id(&conn, media_id)?.ok_or_else(|| NOT_FOUND)?;
            let kit = models::Kit::by_id(&conn, media.get_kit_id())?.ok_or_else(|| NOT_FOUND)?;

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
            .get(&kit.serial, &media.id.to_hyphenated().to_string())
            .await
            .unwrap();

        Ok(ResponseBuilder::ok()
            .attachment_filename(&media.name)
            .stream(media.r#type, stream))
    }

    warp::get()
        .and(warp::path!("media" / Uuid / "content"))
        .and(authentication::option_by_token())
        .and_then(move |media_id: Uuid, user_id: Option<models::UserId>| {
            implementation(
                pg.clone(),
                object_store.clone(),
                user_id,
                models::MediaId(media_id),
            )
            .never_error()
        })
}
