#![recursion_limit = "1024"]

mod subscribers;
mod types;
mod web_socket_session;

use subscribers::Subscribers;
pub use types::RawMeasurement;

use jsonrpc_core::MetaIoHandler;
use jsonrpc_core::{futures as futuresOne, Params, Value};
use jsonrpc_pubsub::typed::{Sink, Subscriber};
use jsonrpc_pubsub::{PubSubHandler, Session, SubscriptionId};
use jsonrpc_server_utils::tokio;
use log::{debug, trace};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use warp::{filters::BoxedFilter, Filter};

use futuresOne::future::Future as FutureOne;

type PeripheralQuantityType = (i32, i32);

#[derive(Clone)]
struct WebSocketHandler {
    executor: tokio::runtime::TaskExecutor,
    raw_measurement_subscriptions: Arc<RwLock<HashMap<String, Subscribers<Sink<Value>>>>>,
    raw_measurement_buffer:
        Arc<RwLock<HashMap<String, HashMap<PeripheralQuantityType, RawMeasurement>>>>,
}

impl WebSocketHandler {
    fn new(executor: tokio::runtime::TaskExecutor) -> Self {
        Self {
            executor,
            raw_measurement_subscriptions: Arc::new(RwLock::new(HashMap::default())),
            raw_measurement_buffer: Arc::new(RwLock::new(HashMap::default())),
        }
    }

    fn buffer_raw_measurement(&self, kit_serial: String, raw_measurement: RawMeasurement) {
        let mut buffer = self.raw_measurement_buffer.write().unwrap();
        let index = (raw_measurement.peripheral, raw_measurement.quantity_type);

        buffer
            .entry(kit_serial)
            .or_default()
            .insert(index, raw_measurement);
    }

    fn publish_raw_measurement(&self, kit_serial: String, raw_measurement: RawMeasurement) {
        let subscriptions = self.raw_measurement_subscriptions.read().unwrap();

        let subscribers: Option<&Subscribers<Sink<Value>>> = subscriptions.get(&kit_serial);
        if let Some(subscribers) = subscribers {
            let value = serde_json::to_value(raw_measurement.clone()).unwrap();
            for subscriber in subscribers.values() {
                self.executor.spawn(
                    subscriber
                        .notify(Ok(value.clone()))
                        .map(|_| ())
                        .map_err(|_| ()),
                );
            }
        }

        self.buffer_raw_measurement(kit_serial, raw_measurement);
    }

    fn add_raw_measurement_subscriber(&self, kit_serial: String, subscriber: Subscriber<Value>) {
        let buffer = self.raw_measurement_buffer.read().unwrap();
        let resend: Vec<RawMeasurement> = match buffer.get(&kit_serial) {
            Some(pqt_raw_measurements) => pqt_raw_measurements.values().cloned().collect(),
            None => vec![],
        };

        let mut subscriptions = self.raw_measurement_subscriptions.write().unwrap();
        let subscribers = subscriptions.entry(kit_serial).or_default();
        let id = subscribers.add(subscriber);

        let sink = id.and_then(|id| subscribers.get(&id));

        // Resend buffered raw measurements to new connection.
        if let Some(sink) = sink {
            for raw_measurement in resend {
                self.executor.spawn(
                    sink.notify(Ok(serde_json::to_value(raw_measurement).unwrap()))
                        .map(|_| ())
                        .map_err(|_| ()),
                )
            }
        }
    }

    fn remove_raw_measurement_subscriber(&self, id: SubscriptionId) {
        trace!("Raw measurement subscriber removed: {:?}", id);
    }
}

pub struct WebSocketPublisher {
    // TODO: perhaps communicate through a channel if the RwLocks become a bottleneck
    web_socket_handler: WebSocketHandler,
}

impl WebSocketPublisher {
    pub fn publish_raw_measurement(&mut self, kit_serial: String, raw_measurement: RawMeasurement) {
        self.web_socket_handler
            .publish_raw_measurement(kit_serial, raw_measurement);
    }
}

/// Runs a JSON-RPC server on top of a Warp WebSocket filter.
/// An executor for handling messages in run in another thread.
///
/// Returns a Warp filter and a handle to publish to subscriptions.
pub fn run() -> (BoxedFilter<(impl warp::Reply,)>, WebSocketPublisher) {
    let mut runtime = tokio::runtime::Builder::new().build().unwrap();

    let web_socket_handler = WebSocketHandler::new(runtime.executor());

    std::thread::spawn(move || runtime.block_on(futuresOne::future::empty::<(), ()>()));

    let mut io = PubSubHandler::new(MetaIoHandler::default());
    io.add_subscription(
        "rawMeasurements",
        ("subscribe_rawMeasurements", {
            let web_socket_handler = web_socket_handler.clone();
            move |params: Params, _: Arc<Session>, subscriber: jsonrpc_pubsub::Subscriber| {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct SubParams {
                    kit_serial: String,
                }

                match params.parse::<SubParams>() {
                    Ok(sub_params) => {
                        let subscriber = Subscriber::new(subscriber);
                        web_socket_handler
                            .add_raw_measurement_subscriber(sub_params.kit_serial, subscriber);
                    }
                    Err(_) => {}
                }
            }
        }),
        ("unsubscribe_rawMeasurements", {
            let web_socket_handler = web_socket_handler.clone();
            move |id: SubscriptionId, _| {
                web_socket_handler.remove_raw_measurement_subscriber(id);
                futuresOne::future::ok(Value::Bool(true))
            }
        }),
    );
    let io_handler: MetaIoHandler<Arc<Session>> = io.into();

    let num_sockets = Arc::new(Mutex::new(0usize));
    let filter = warp::ws()
        .map(move |ws: warp::ws::Ws| {
            let mut num_sockets = num_sockets.lock().unwrap();
            let socket_id: usize = *num_sockets;
            *num_sockets += 1;
            let io_handler = io_handler.clone();

            trace!("Websocket {} connecting", socket_id);
            ws.on_upgrade(move |web_socket| {
                async move {
                    debug!("Websocket {} upgraded", socket_id);
                    web_socket_session::handle_session(socket_id, web_socket, io_handler).await;
                    debug!("WebSocket {} stopped", socket_id);
                }
            })
        })
        .boxed();

    let publisher = WebSocketPublisher {
        web_socket_handler: web_socket_handler.clone(),
    };

    (filter, publisher)
}
