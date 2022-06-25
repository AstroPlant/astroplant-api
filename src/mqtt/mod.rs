use crate::database::PgPool;
use crate::{helpers, models, problem, views};

use astroplant_mqtt::{ConnectionBuilder, Message, RpcError};
use futures::channel::mpsc;
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use std::convert::TryFrom;

async fn upload_media(
    pg_pool: PgPool,
    object_store: astroplant_object::ObjectStore,
    media: astroplant_mqtt::Media,
) {
    let implementation = move || async move {
        let astroplant_mqtt::Media {
            id,
            kit_serial,
            datetime,
            peripheral,
            name,
            r#type,
            data,
            metadata,
        } = media;

        // TODO: handle errors.
        let object_name = id.to_hyphenated().to_string();
        let size = i64::try_from(data.len()).map_err(|_| problem::INTERNAL_SERVER_ERROR)?;

        tracing::trace!(
            "Uploading media for kit {}: file {}, name '{}', type '{}', {} byte(s)",
            kit_serial,
            object_name,
            name,
            r#type,
            size,
        );

        let conn = pg_pool.clone().get().await?;
        let peripheral = helpers::threadpool(move || {
            models::Peripheral::by_id(&conn, models::PeripheralId(peripheral))
        })
        .await?
        .ok_or_else(|| problem::NOT_FOUND)?;

        if let Err(err) = object_store
            .put(&kit_serial, &object_name, data, r#type.clone())
            .await
        {
            tracing::warn!(
                    "Failed to upload media for kit {}: file {}, name '{}', type '{}', {} byte(s). Error: {:?}",
                    kit_serial,
                    object_name,
                    name,
                    r#type,
                    size,
                    err,
                );
        };

        let conn = pg_pool.get().await?;
        if let Err(_) = helpers::threadpool(move || {
            let new = models::NewMedia::new(
                id,
                peripheral.get_id(),
                peripheral.get_kit_id(),
                peripheral.get_kit_configuration_id(),
                datetime,
                name,
                r#type,
                metadata,
                size,
            );
            new.create(&conn)
        })
        .await
        {
            // TODO: Failed to insert into database, remove object
        }

        Ok::<(), problem::Problem>(())
    };

    if let Err(_) = implementation().await {
        tracing::warn!("encountered a problem when uploading media");
    }
}

struct Handler_ {
    pg_pool: PgPool,
}

#[async_trait::async_trait]
impl astroplant_mqtt::ServerRpcHandler for Handler_ {
    async fn version(&self) -> Result<String, RpcError> {
        tracing::trace!("RPC: handling version request");

        Ok(super::VERSION.to_owned())
    }

    async fn get_active_configuration(
        &self,
        kit_serial: String,
    ) -> Result<Option<serde_json::Value>, RpcError> {
        tracing::trace!("RPC: handling getActiveConfiguration request");

        let conn = self
            .pg_pool
            .clone()
            .get()
            .await
            .map_err(|_| RpcError::Other)?;
        let configuration: Option<_> = helpers::threadpool(move || {
            let kit =
                match models::Kit::by_serial(&conn, kit_serial).map_err(|_| RpcError::Other)? {
                    Some(kit) => kit,
                    None => return Ok(None),
                };
            let configuration =
                match models::KitConfiguration::active_configuration_of_kit(&conn, &kit)
                    .map_err(|_| RpcError::Other)?
                {
                    Some(configuration) => configuration,
                    None => return Ok(None),
                };
            let peripherals_with_definitions =
                models::Peripheral::peripherals_with_definitions_of_kit_configuration(
                    &conn,
                    &configuration,
                )
                .map_err(|_| RpcError::Other)?;

            let configuration = views::KitConfiguration::from(configuration);
            let peripherals_with_definitions: Vec<_> = peripherals_with_definitions
                .into_iter()
                .map(|(peripheral, definition)| {
                    let definition = views::PeripheralDefinition::from(definition);
                    views::Peripheral::from(peripheral).with_definition(definition)
                })
                .collect();

            Ok(Some(
                configuration.with_peripherals(peripherals_with_definitions),
            ))
        })
        .await?;

        Ok(configuration.map(|configuration| serde_json::to_value(configuration).unwrap()))
    }

    async fn get_quantity_types(&self) -> Result<Vec<serde_json::Value>, RpcError> {
        tracing::trace!("RPC: handling getQuantityTypes request");

        let conn = self
            .pg_pool
            .clone()
            .get()
            .await
            .map_err(|_| RpcError::Other)?;

        let quantity_types: Vec<_> = helpers::threadpool(move || {
            let quantity_types = models::QuantityType::all(&conn)
                .map_err(|_| RpcError::Other)?
                .into_iter()
                .map(|quantity_type| views::QuantityType::from(quantity_type))
                .map(|quantity_type| serde_json::to_value(quantity_type).unwrap())
                .collect();

            Ok(quantity_types)
        })
        .await?;

        Ok(quantity_types)
    }
}

/// Must be called from within a Tokio runtime.
pub fn run(
    pg_pool: PgPool,
    object_store: astroplant_object::ObjectStore,
) -> (
    mpsc::Receiver<astroplant_mqtt::RawMeasurement>,
    astroplant_mqtt::KitsRpc,
) {
    let (mut raw_measurement_sender, raw_measurement_receiver) = mpsc::channel(128);

    let mut builder = ConnectionBuilder::new(
        std::env::var("MQTT_HOST").unwrap_or(crate::DEFAULT_MQTT_HOST.to_owned()),
        std::env::var("MQTT_PORT")
            .map_err(|_| ())
            .and_then(|port| port.parse().map_err(|_| ()))
            .unwrap_or(crate::DEFAULT_MQTT_PORT),
    );

    if let Ok(username) = std::env::var("MQTT_USERNAME") {
        builder = builder.with_credentials(
            username,
            std::env::var("MQTT_PASSWORD").unwrap_or("".to_string()),
        );
    }

    let builder = builder.with_server_rpc_handler(Handler_ {
        pg_pool: pg_pool.clone(),
    });

    let (connection, kits_rpc) = builder.create();

    tokio::spawn(async move {
        let mut stream = connection.into_stream();
        while let Some(msg) = stream.next().await {
            match msg {
                Ok(Message::RawMeasurement(measurement)) => {
                    if let Err(_) = raw_measurement_sender.send(measurement).await {
                        break;
                    }
                }
                Ok(Message::Media(media)) => {
                    upload_media(pg_pool.clone(), object_store.clone(), media).await;
                }
                Err(err) => {
                    tracing::warn!("An MQTT error was encountered: {:?}", err)
                }
                _ => {}
            }
        }
    });

    (raw_measurement_receiver, kits_rpc)
}
