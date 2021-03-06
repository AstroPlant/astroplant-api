use crate::database::PgPool;
use crate::{helpers, models, problem, views};

use astroplant_mqtt::{MqttApiMessage, ServerRpcRequest};
use futures::channel::{mpsc, oneshot};
use futures::future::FutureExt;
use futures::sink::SinkExt;
use std::convert::TryFrom;
use tokio::runtime;

#[derive(Debug)]
enum Error {
    PgPool,
    Internal,
}

struct Handler {
    pg_pool: PgPool,
    object_store: astroplant_object::ObjectStore,
    runtime_handle: runtime::Handle,
    raw_measurement_sender: mpsc::Sender<astroplant_mqtt::RawMeasurement>,
}

impl Handler {
    pub fn new(
        pg_pool: PgPool,
        object_store: astroplant_object::ObjectStore,
        runtime_handle: runtime::Handle,
        raw_measurement_sender: mpsc::Sender<astroplant_mqtt::RawMeasurement>,
    ) -> Self {
        Self {
            pg_pool,
            object_store,
            runtime_handle,
            raw_measurement_sender,
        }
    }

    async fn get_active_configuration(
        pg: PgPool,
        kit_serial: String,
        response: oneshot::Sender<Option<serde_json::Value>>,
    ) -> Result<(), Error> {
        tracing::trace!("handling getActiveConfiguration request for {}", kit_serial);

        let conn = pg.get().await.map_err(|_| Error::PgPool)?;
        let configuration: Option<_> = helpers::threadpool(move || {
            let kit =
                match models::Kit::by_serial(&conn, kit_serial).map_err(|_| Error::Internal)? {
                    Some(kit) => kit,
                    None => return Ok(None),
                };
            let configuration =
                match models::KitConfiguration::active_configuration_of_kit(&conn, &kit)
                    .map_err(|_| Error::Internal)?
                {
                    Some(configuration) => configuration,
                    None => return Ok(None),
                };
            let peripherals_with_definitions =
                models::Peripheral::peripherals_with_definitions_of_kit_configuration(
                    &conn,
                    &configuration,
                )
                .map_err(|_| Error::Internal)?;

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

        let _ = response
            .send(configuration.map(|configuration| serde_json::to_value(configuration).unwrap()));
        Ok(())
    }

    async fn get_quantity_types(
        pg: PgPool,
        response: oneshot::Sender<Vec<serde_json::Value>>,
    ) -> Result<(), Error> {
        tracing::trace!("handling getQuantityTypes request");

        let conn = pg.get().await.map_err(|_| Error::PgPool)?;
        let quantity_types: Vec<_> = helpers::threadpool(move || {
            let quantity_types = models::QuantityType::all(&conn)
                .map_err(|_| Error::Internal)?
                .into_iter()
                .map(|quantity_type| views::QuantityType::from(quantity_type))
                .map(|quantity_type| serde_json::to_value(quantity_type).unwrap())
                .collect();

            Ok(quantity_types)
        })
        .await?;

        let _ = response.send(quantity_types);
        Ok(())
    }

    fn server_rpc_request(&mut self, request: ServerRpcRequest) {
        use ServerRpcRequest::*;

        match request {
            Version { response } => {
                let _ = response.send(super::VERSION.to_owned());
            }
            GetActiveConfiguration {
                kit_serial,
                response,
            } => {
                self.runtime_handle.spawn(
                    Self::get_active_configuration(self.pg_pool.clone(), kit_serial, response)
                        .map(|_| ()),
                );
            }
            GetQuantityTypes { response } => {
                self.runtime_handle
                    .spawn(Self::get_quantity_types(self.pg_pool.clone(), response).map(|_| ()));
            }
        }
    }

    async fn send<T>(mut sender: mpsc::Sender<T>, val: T) {
        // TODO: handle errors.
        let _ = sender.send(val).await;
    }

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

            let naive = chrono::NaiveDateTime::from_timestamp(
                i64::try_from(datetime / 1000).map_err(|_| problem::INTERNAL_SERVER_ERROR)?,
                0,
            );
            let datetime: chrono::DateTime<chrono::Utc> =
                chrono::DateTime::from_utc(naive, chrono::Utc);

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

            let _ = object_store
                .put(&kit_serial, &object_name, data, r#type.clone())
                .await;
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

    pub fn run(
        &mut self,
        message_receiver: crossbeam::channel::Receiver<astroplant_mqtt::MqttApiMessage>,
    ) {
        for message in message_receiver {
            match message {
                MqttApiMessage::ServerRpcRequest(request) => self.server_rpc_request(request),
                MqttApiMessage::RawMeasurement(measurement) => {
                    tracing::trace!("Received measurement: {:?}", measurement);
                    self.runtime_handle
                        .spawn(Self::send(self.raw_measurement_sender.clone(), measurement));
                }
                MqttApiMessage::Media(media) => {
                    tracing::trace!("Received media: {:?}", media.name);
                    self.runtime_handle.spawn(Self::upload_media(
                        self.pg_pool.clone(),
                        self.object_store.clone(),
                        media,
                    ));
                }
                _ => {}
            }
        }

        tracing::debug!("MQTT handler stopped");
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
    let (raw_measurement_sender, raw_measurement_receiver) = mpsc::channel(128);

    let (message_receiver, kits_rpc) = astroplant_mqtt::run(
        std::env::var("MQTT_HOST").unwrap_or(crate::DEFAULT_MQTT_HOST.to_owned()),
        std::env::var("MQTT_PORT")
            .map_err(|_| ())
            .and_then(|port| port.parse().map_err(|_| ()))
            .unwrap_or(crate::DEFAULT_MQTT_PORT),
        std::env::var("MQTT_USERNAME").unwrap_or(crate::DEFAULT_MQTT_USERNAME.to_owned()),
        std::env::var("MQTT_PASSWORD").unwrap_or(crate::DEFAULT_MQTT_PASSWORD.to_owned()),
    );

    let runtime_handle = runtime::Handle::current();
    std::thread::spawn(move || {
        let mut handler = Handler::new(
            pg_pool,
            object_store,
            runtime_handle,
            raw_measurement_sender,
        );
        handler.run(message_receiver);
    });

    (raw_measurement_receiver, kits_rpc)
}
