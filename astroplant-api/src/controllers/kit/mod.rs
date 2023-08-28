use axum::extract::Path;
use axum::Extension;
use serde::{Deserialize, Serialize};

use crate::database::PgPool;
use crate::problem::{self, Problem};
use crate::response::{Response, ResponseBuilder};
use crate::utils::deserialize_some;
use crate::{helpers, models, views};

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
