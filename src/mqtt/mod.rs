use super::{helpers, models, views, PgPool, PgPooled};

use astroplant_mqtt::{MqttApiMessage, ServerRpcRequest};
use futures::channel::oneshot;
use futures::future::{FutureExt, TryFutureExt};
use tokio::runtime::{Runtime, TaskExecutor};

#[derive(Debug)]
enum Error {
    PgPool,
    Internal,
}

struct Handler {
    pg_pool: PgPool,
    executor: TaskExecutor,
}

impl Handler {
    pub fn new(pg_pool: PgPool, executor: TaskExecutor) -> Self {
        Self { pg_pool, executor }
    }

    async fn get_active_configuration(
        pg_pool: PgPool,
        kit_serial: String,
        response: oneshot::Sender<Option<serde_json::Value>>,
    ) -> Result<(), Error> {
        trace!("handling getActiveConfiguration request for {}", kit_serial);

        helpers::threadpool(move || pg_pool.get().map_err(|_| Error::PgPool))
            .and_then(|conn: PgPooled| {
                helpers::threadpool(move || {
                    println!("getting for kit: {}", kit_serial);
                    let kit = match models::Kit::by_serial(&conn, kit_serial)
                        .map_err(|_| Error::Internal)?
                    {
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
                    let peripherals =
                        models::Peripheral::peripherals_of_kit_configuration(&conn, &configuration)
                            .map_err(|_| Error::Internal)?;

                    let configuration = views::KitConfiguration::from(configuration);
                    let peripherals: Vec<_> = peripherals
                        .into_iter()
                        .map(views::Peripheral::from)
                        .collect();

                    Ok(Some(configuration.with_peripherals(peripherals)))
                })
            })
            .map_ok(|configuration| {
                let configuration =
                    configuration.map(|configuration| serde_json::to_value(configuration).unwrap());
                let _ = response.send(configuration);
            })
            .await
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
                self.executor.spawn(
                    Self::get_active_configuration(self.pg_pool.clone(), kit_serial, response)
                        .map(|_| ()),
                );
            }
        }
    }

    pub fn run(&mut self) {
        let (message_receiver, _kits_rpc) = astroplant_mqtt::run();
        for message in message_receiver {
            match message {
                MqttApiMessage::ServerRpcRequest(request) => self.server_rpc_request(request),
                MqttApiMessage::RawMeasurement(measurement) => {
                    println!("Received measurement: {:?}", measurement);
                }
                _ => {}
            }
        }
    }
}

pub fn run(pg_pool: PgPool) {
    let (thread_pool_handle_sender, thread_pool_handle_receiver) = oneshot::channel::<()>();
    let runtime = Runtime::new().unwrap();
    let executor = runtime.executor();

    std::thread::spawn(move || runtime.block_on(thread_pool_handle_receiver));

    let mut handler = Handler::new(pg_pool, executor);
    handler.run();

    thread_pool_handle_sender.send(()).unwrap();
}
