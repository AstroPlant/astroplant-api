use axum::extract::Path;
use axum::Extension;
use futures::{StreamExt, TryStreamExt};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPool as SqlxPgPool;
use std::collections::HashMap;
use tokio::io::AsyncWriteExt;

use crate::database::PgPool;
use crate::problem::{self, Problem};
use crate::response::{Response, ResponseBuilder};
use crate::{helpers, models, views};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Query {
    token: String,
    from: Option<chrono::DateTime<chrono::Utc>>,
    to: Option<chrono::DateTime<chrono::Utc>>,
    configuration: Option<i32>,
}

#[derive(Serialize, Deserialize)]
pub struct DownloadToken {
    action: String,
    kit_serial: String,
}

async fn write_aggregates<W: tokio::io::AsyncWrite + Unpin>(
    writer: &mut W,
    sqlx_pg: &SqlxPgPool,
    kit: &models::Kit,
    filter: &Query,
) -> anyhow::Result<()> {
    #[derive(Deserialize)]
    struct Values(HashMap<String, f64>);

    // Get the (unique) aggregate types in this dataset. Each aggregate type gets a column in the
    // CSV file.
    let aggregate_keys: Vec<String> = sqlx::query!(
        "
SELECT DISTINCT json_object_keys(values) as key
FROM aggregate_measurements
WHERE kit_id=$1
AND ($2 OR kit_configuration_id=$3)
AND ($4 OR datetime_start>=$5)
AND ($6 OR datetime_end<=$7)
            ",
        kit.id,
        filter.configuration.is_none(),
        filter.configuration,
        filter.from.is_none(),
        filter.from,
        filter.to.is_none(),
        filter.to,
    )
    .fetch(sqlx_pg)
    .try_filter_map(|key| async move { Ok(key.key) })
    .try_collect()
    .await?;

    // Write the CSV header
    let mut aggregate_keys_string = if aggregate_keys.is_empty() {
        "".to_string()
    } else {
        ",".to_string()
    };
    aggregate_keys_string.push_str(&aggregate_keys.join(","));

    writer
        .write_all_buf(
            &mut format!(
                "datetimeStart,datetimeEnd,peripheral,kitConfiguration,quantityType{}\n",
                aggregate_keys_string
            )
            .as_bytes(),
        )
        .await?;

    // Get the CSV data.
    let mut aggregates = sqlx::query!(
        "
SELECT datetime_start,datetime_end,peripheral_id, kit_configuration_id, quantity_type_id, values
FROM aggregate_measurements
WHERE kit_id=$1
AND ($2 OR kit_configuration_id=$3)
AND ($4 OR datetime_start>=$5)
AND ($6 OR datetime_end<=$7)
        ",
        kit.id,
        filter.configuration.is_none(),
        filter.configuration,
        filter.from.is_none(),
        filter.from,
        filter.to.is_none(),
        filter.to,
    )
    .fetch(sqlx_pg);
    let mut values_string: String = String::new();

    while let Some(Ok(aggregate)) = aggregates.next().await {
        values_string.clear();
        let values: Values = serde_json::from_value(aggregate.values)?;

        for key in aggregate_keys.iter() {
            values_string.push_str(",");
            values_string.push_str(
                &values
                    .0
                    .get(key)
                    .map(|value| value.to_string())
                    .unwrap_or("".to_string()),
            );
        }
        writer
            .write_all_buf(
                &mut format!(
                    "{},{},{},{},{}{}\n",
                    aggregate
                        .datetime_start
                        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                    aggregate
                        .datetime_end
                        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                    aggregate.peripheral_id,
                    aggregate.kit_configuration_id,
                    aggregate.quantity_type_id,
                    values_string,
                )
                .as_bytes(),
            )
            .await?;
    }

    Ok(())
}

async fn write_configs<W: tokio::io::AsyncWrite + Unpin>(
    writer: &mut W,
    pg: PgPool,
    kit: &models::Kit,
) -> anyhow::Result<()> {
    let conn = pg.get().await?;

    let kit_configurations = models::KitConfiguration::configurations_of_kit(&conn, &kit)?;
    let kit_peripherals = models::Peripheral::peripherals_of_kit(&conn, &kit)?;
    let mut kit_peripherals: HashMap<i32, Vec<views::Peripheral>> = kit_peripherals
        .into_iter()
        .map(|p| (p.kit_configuration_id, views::Peripheral::from(p)))
        .into_group_map();
    let kit_configurations_with_peripherals: Vec<
        views::KitConfigurationWithPeripherals<views::Peripheral>,
    > = kit_configurations
        .into_iter()
        .map(views::KitConfiguration::from)
        .map(|c| {
            let id = c.id;
            c.with_peripherals(kit_peripherals.remove(&id).unwrap_or_default())
        })
        .collect();

    let config = serde_json::to_string_pretty(&kit_configurations_with_peripherals)?;
    writer.write_all(config.as_bytes()).await?;

    Ok(())
}

async fn write_zip<W: tokio::io::AsyncWrite + Unpin>(
    writer: W,
    pg: PgPool,
    sqlx_pg: SqlxPgPool,
    now: chrono::DateTime<chrono::Utc>,
    kit: models::Kit,
    filter: Query,
) -> anyhow::Result<()> {
    let now = zipit::FileDateTime::from_chrono_datetime(now);
    let mut zip = zipit::Archive::new(writer);

    // Write the kit configurations to the archive.
    let (mut reader, mut writer) = tokio::io::duplex(1024);
    tokio::join!(
        async {
            let _ = zip
                .append("configurations.json".to_string(), now, &mut reader)
                .await;
        },
        async {
            let _ = write_configs(&mut writer, pg, &kit).await;
            drop(writer);
        }
    );

    // Write aggregate measurements to the archive.
    let (mut reader, mut writer) = tokio::io::duplex(1024);
    tokio::join!(
        async {
            let _ = zip
                .append("aggregate_measurements.csv".to_string(), now, &mut reader)
                .await;
        },
        async {
            let _ = write_aggregates(&mut writer, &sqlx_pg, &kit, &filter).await;
            drop(writer);
        }
    );

    zip.finalize().await?;

    Ok(())
}

/// Handles the `POST /kits/{kitSerial}/archive` route. This authenticates and authorizes the
/// client for an authorized download (necessary to download data of non-public kits). The client
/// is given a short-lived token that allows it to initiate a download by passing that token as a
/// query parameter. For simplicity we use the same scheme for public kits, that way the download
/// handler simply always checks the token.
pub async fn archive_authorize(
    user_id: Option<models::UserId>,
    Path(kit_serial): Path<String>,
    Extension(pg): Extension<PgPool>,
) -> Result<Response, Problem> {
    let (_, _, kit) = helpers::fut_kit_permission_or_forbidden(
        pg.clone(),
        user_id,
        kit_serial.clone(),
        crate::authorization::KitAction::View,
    )
    .await?;

    let token = DownloadToken {
        action: "kit-archive-download".to_owned(),
        kit_serial: kit.serial,
    };

    let token_signer = crate::TOKEN_SIGNER.get().unwrap();
    let token = token_signer.create_arbitrary_token(token, chrono::Duration::seconds(30));

    let response = ResponseBuilder::ok();
    Ok(response.body(token))
}

/// Handles the `GET /kits/{kitSerial}/archive` route.
pub async fn archive(
    Path(kit_serial): Path<String>,
    crate::extract::Query(query): crate::extract::Query<Query>,
    Extension(pg): Extension<PgPool>,
    Extension(sqlx_pg): Extension<SqlxPgPool>,
) -> Result<Response, Problem> {
    let token_signer = crate::TOKEN_SIGNER.get().unwrap();
    let token: DownloadToken = token_signer
        .decode_arbitrary_token(&query.token)
        .map_err(|_| problem::FORBIDDEN)?;

    if token.action != "kit-archive-download" || token.kit_serial != kit_serial {
        return Err(problem::FORBIDDEN);
    }
    drop(kit_serial);

    let conn = pg.clone().get().await?;
    let s = token.kit_serial.clone();
    let kit =
        crate::helpers::threadpool(move || crate::models::Kit::by_serial(&conn, s).ok().flatten())
            .await
            .ok_or(problem::NOT_FOUND)?;

    let (archive, api) = tokio::io::duplex(2048);

    let now = chrono::Utc::now();
    tokio::spawn(write_zip(
        archive,
        pg.to_owned(),
        sqlx_pg.to_owned(),
        now,
        kit,
        query,
    ));

    let api = tokio_util::io::ReaderStream::new(api);

    // Get the datetime string, but remove colons (these are not always safe in filenames).
    let mut now_string = now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    now_string.retain(|c| c != ':');
    Ok(ResponseBuilder::ok()
        .attachment_filename(&format!("{}_{}.zip", token.kit_serial, now_string))
        .stream("application/zip".to_string(), api.boxed()))
}
