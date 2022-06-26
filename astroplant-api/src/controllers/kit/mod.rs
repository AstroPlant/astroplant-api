use futures::future::FutureExt;
use serde::{Deserialize, Serialize};
use warp::{filters::BoxedFilter, path, Filter, Rejection};

use crate::database::PgPool;
use crate::problem::{self, AppResult, Problem};
use crate::response::{Response, ResponseBuilder};
use crate::{authentication, helpers, models, views};

pub fn router(pg: PgPool) -> BoxedFilter<(AppResult<Response>,)> {
    //impl Filter<Extract = (Response,), Error = Rejection> + Clone {
    tracing::trace!("Setting up kits router.");

    (warp::get().and(kit_by_serial(pg.clone())))
        .or(warp::post().and(reset_password(pg.clone())))
        .unify()
        .or(warp::path::end().and(warp::get()).and(kits(pg.clone())))
        .unify()
        .or(warp::path::end()
            .and(warp::post())
            .and(create_kit(pg.clone())))
        .unify()
        .or(patch_kit(pg))
        .unify()
        .boxed()
}

#[derive(Deserialize)]
struct CursorPage {
    after: Option<i32>,
}

/// Handles the `GET /kits/?after=afterId` route.
pub fn kits(
    pg: PgPool,
) -> impl Filter<Extract = (AppResult<Response>,), Error = Rejection> + Clone {
    async fn implementation(pg: PgPool, cursor: CursorPage) -> AppResult<Response> {
        let conn = pg.get().await?;
        let kits = helpers::threadpool(move || {
            models::Kit::cursor_page(&conn, cursor.after, 100).map(|kits| {
                kits.into_iter()
                    .map(|kit| views::Kit::from(kit))
                    .collect::<Vec<_>>()
            })
        })
        .await?;

        let next_page_uri = kits.last().map(|last| format!("/kits?after={}", last.id));
        let mut response_builder = ResponseBuilder::ok();
        if let Some(next_page_uri) = next_page_uri {
            response_builder = response_builder.next_page_uri(next_page_uri);
        }
        Ok(response_builder.body(kits))
    }

    warp::query::query::<CursorPage>()
        .and_then(move |cursor: CursorPage| implementation(pg.clone(), cursor).never_error())
}

/// Handles the `GET /kits/{kitSerial}` route.
pub fn kit_by_serial(
    pg: PgPool,
) -> impl Filter<Extract = (AppResult<Response>,), Error = Rejection> + Clone {
    async fn implementation(
        pg: PgPool,
        kit_serial: String,
        user_id: Option<models::UserId>,
    ) -> AppResult<Response> {
        let (_, _, kit) = helpers::fut_kit_permission_or_forbidden(
            pg,
            user_id,
            kit_serial,
            crate::authorization::KitAction::View,
        )
        .await?;
        Ok(ResponseBuilder::ok().body(views::Kit::from(kit)))
    }

    path!(String)
        .and(authentication::option_by_token())
        .and_then(move |kit_serial: String, user_id: Option<models::UserId>| {
            implementation(pg.clone(), kit_serial, user_id).never_error()
        })
}

/// Handles the `POST /kits/{kitSerial}/password` route.
pub fn reset_password(
    pg: PgPool,
) -> impl Filter<Extract = (AppResult<Response>,), Error = Rejection> + Clone {
    async fn implementation(
        pg: PgPool,
        kit_serial: String,
        user_id: Option<models::UserId>,
    ) -> AppResult<Response> {
        let (_, _, kit) = helpers::fut_kit_permission_or_forbidden(
            pg.clone(),
            user_id,
            kit_serial,
            crate::authorization::KitAction::ResetPassword,
        )
        .await?;
        let conn = pg.get().await?;
        let password = helpers::threadpool(move || {
            let (update_kit, password) =
                models::UpdateKit::unchanged_for_id(kit.id).reset_password();
            update_kit.update(&conn)?;
            Ok::<_, Problem>(password)
        })
        .await?;
        Ok(ResponseBuilder::ok().body(password))
    }

    path!(String / "password")
        .and(authentication::option_by_token())
        .and_then(move |kit_serial: String, user_id: Option<models::UserId>| {
            implementation(pg.clone(), kit_serial, user_id).never_error()
        })
}

/// Handles the `POST /kits` route.
pub fn create_kit(
    pg: PgPool,
) -> impl Filter<Extract = (AppResult<Response>,), Error = Rejection> + Clone {
    use bigdecimal::{BigDecimal, FromPrimitive};
    use diesel::Connection;
    use validator::Validate;

    #[derive(Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    struct Kit {
        name: Option<String>,
        description: Option<String>,
        latitude: Option<f64>,
        longitude: Option<f64>,
        privacy_public_dashboard: bool,
        privacy_show_on_map: bool,
    }

    #[derive(Serialize, Debug)]
    #[serde(rename_all = "camelCase")]
    struct Created {
        kit_serial: String,
        password: String,
    }

    async fn implementation(pg: PgPool, user_id: models::UserId, kit: Kit) -> AppResult<Response> {
        let (new_kit, password) = models::NewKit::new_with_generated_password(
            kit.name,
            kit.description,
            kit.latitude.and_then(|l| BigDecimal::from_f64(l)),
            kit.longitude.and_then(|l| BigDecimal::from_f64(l)),
            kit.privacy_public_dashboard,
            kit.privacy_show_on_map,
        );

        if let Err(validation_errors) = new_kit.validate() {
            let invalid_parameters = problem::InvalidParameters::from(validation_errors);
            return Err(problem::Problem::InvalidParameters { invalid_parameters });
        };

        let conn = pg.get().await?;
        helpers::threadpool(move || {
            conn.transaction(|| {
                let created_kit: models::Kit = new_kit.create(&conn)?;
                let kit_serial = created_kit.serial;
                tracing::debug!("Created kit \"{}\"", kit_serial);
                let kit_id = models::KitId(created_kit.id);

                models::NewKitMembership::new(user_id, kit_id, true, true).create(&conn)?;

                let response = ResponseBuilder::created().body(Created {
                    kit_serial,
                    password,
                });

                Ok(response)
            })
        })
        .await
    }

    authentication::by_token()
        .and(crate::helpers::deserialize())
        .and_then(move |user_id: models::UserId, kit: Kit| {
            implementation(pg.clone(), user_id, kit).never_error()
        })
}

/// Handles the `PATCH /kits/{kitSerial}` route.
fn patch_kit(
    pg: PgPool,
) -> impl Filter<Extract = (AppResult<Response>,), Error = Rejection> + Clone {
    use bigdecimal::{BigDecimal, FromPrimitive};

    use crate::utils::deserialize_some;

    #[derive(Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    struct KitPatch {
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

    async fn implementation(
        pg: PgPool,
        kit_serial: String,
        user_id: Option<models::UserId>,
        kit_patch: KitPatch,
    ) -> AppResult<Response> {
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
            latitude: kit_patch
                .latitude
                .map(|l| l.and_then(|l| BigDecimal::from_f64(l))),
            longitude: kit_patch
                .longitude
                .map(|l| l.and_then(|l| BigDecimal::from_f64(l))),
            privacy_public_dashboard: kit_patch.privacy_public_dashboard,
            privacy_show_on_map: kit_patch.privacy_show_on_map,
            password_hash: None,
        };

        let conn = pg.get().await?;
        helpers::threadpool(move || {
            let patched_kit = update_kit.update(&conn)?;
            Ok(ResponseBuilder::ok().body(views::Kit::from(patched_kit)))
        })
        .await
    }

    warp::patch()
        .and(path!(String))
        .and(authentication::option_by_token())
        .and(crate::helpers::deserialize())
        .and_then(
            move |kit_serial: String, user_id: Option<models::UserId>, kit_patch: KitPatch| {
                implementation(pg.clone(), kit_serial, user_id, kit_patch).never_error()
            },
        )
}
