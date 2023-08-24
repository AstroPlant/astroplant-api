use axum::extract::Path;
use axum::Extension;
use serde::{Deserialize, Serialize};

use crate::database::PgPool;
use crate::problem::Problem;
use crate::response::{Response, ResponseBuilder};
use crate::{authorization, helpers, models, views};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Query {
    cursor: Option<String>,
    configuration: Option<i32>,
    peripheral: Option<i32>,
    quantity_type: Option<i32>,
}

/// Handles the `GET /kits/{kitSerial}/aggregate-measurements` route.
pub async fn kit_aggregate_measurements(
    Extension(pg): Extension<PgPool>,
    user_id: Option<models::UserId>,
    Path(kit_serial): Path<String>,
    crate::extract::Query(query): crate::extract::Query<Query>,
) -> Result<Response, Problem> {
    use crate::cursors;
    use std::convert::TryFrom;

    let mut out_query = query.clone();
    let cursor = query.cursor.as_ref().map(|s| s.parse()).transpose()?;
    let base_uri = format!("/kits/{}/aggregate-measurements", kit_serial);

    let (_user, _membership, kit) = helpers::fut_kit_permission_or_forbidden(
        pg.clone(),
        user_id,
        kit_serial,
        authorization::KitAction::View,
    )
    .await?;

    let conn = pg.get().await?;
    let mut response = ResponseBuilder::ok();
    let aggregate_measurements = conn
        .interact_flatten_err(move |conn| {
            models::AggregateMeasurement::page(
                conn,
                kit.get_id(),
                query.configuration,
                query.peripheral,
                query.quantity_type,
                cursor,
            )
        })
        .await?;

    if let Some(next_cursor) =
        cursors::AggregateMeasurements::next_from_page(&aggregate_measurements)
    {
        out_query.cursor = Some(next_cursor.into());
        let next_page_uri = format!(
            "{}?{}",
            base_uri,
            serde_urlencoded::to_string(&out_query).unwrap()
        );
        response = response.link(&next_page_uri, "next");
    }

    let body = aggregate_measurements
        .into_iter()
        .map(views::AggregateMeasurement::try_from)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(response.body(body))
}
