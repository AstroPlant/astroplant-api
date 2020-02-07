use super::{helpers, models, views, PgPool, PgPooled};

use astroplant_mqtt::{MqttApiMessage, ServerRpcRequest};
use futures::channel::{mpsc, oneshot};
use futures::future::FutureExt;
use futures::sink::SinkExt;
use tokio::runtime::{Runtime, Handle};

#[derive(Debug)]
enum Error {
    PgPool,
    Internal,
}

struct Handler {
    pg_pool: PgPool,
    runtime_handle: Handle,
    raw_measurement_sender: mpsc::Sender<astroplant_mqtt::RawMeasurement>,
}

impl Handler {
    pub fn new(
        pg_pool: PgPool,
        runtime_handle: Handle,
        raw_measurement_sender: mpsc::Sender<astroplant_mqtt::RawMeasurement>,
    ) -> Self {
        Self {
            pg_pool,
            runtime_handle,
            raw_measurement_sender,
        }
    }

    async fn get_active_configuration(
        pg_pool: PgPool,
        kit_serial: String,
        response: oneshot::Sender<Option<serde_json::Value>>,
    ) -> Result<(), Error> {
        trace!("handling getActiveConfiguration request for {}", kit_serial);

        let conn: PgPooled =
            helpers::threadpool(move || pg_pool.get().map_err(|_| Error::PgPool)).await?;
        let configuration: Option<_> = helpers::threadpool(move || {
            println!("getting for kit: {}", kit_serial);
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
        pg_pool: PgPool,
        response: oneshot::Sender<Vec<serde_json::Value>>,
    ) -> Result<(), Error> {
        trace!("handling getQuantityTypes request");

        let conn: PgPooled =
            helpers::threadpool(move || pg_pool.get().map_err(|_| Error::PgPool)).await?;
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

    pub fn run(
        &mut self,
        message_receiver: crossbeam::channel::Receiver<astroplant_mqtt::MqttApiMessage>,
    ) {
        for message in message_receiver {
            match message {
                MqttApiMessage::ServerRpcRequest(request) => self.server_rpc_request(request),
                MqttApiMessage::RawMeasurement(measurement) => {
                    println!("Received measurement: {:?}", measurement);
                    self.runtime_handle
                        .spawn(Self::send(self.raw_measurement_sender.clone(), measurement));
                }
                _ => {}
            }
        }
    }
}

pub fn run(
    pg_pool: PgPool,
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

    std::thread::spawn(move || {
        let (thread_pool_handle_sender, thread_pool_handle_receiver) = oneshot::channel::<()>();
        let mut runtime = Runtime::new().unwrap();
        let runtime_handle = runtime.handle().clone();

        std::thread::spawn(move || runtime.block_on(thread_pool_handle_receiver));

        let mut handler = Handler::new(pg_pool, runtime_handle, raw_measurement_sender);
        handler.run(message_receiver);

        thread_pool_handle_sender.send(()).unwrap();
    });

    (raw_measurement_receiver, kits_rpc)
}
