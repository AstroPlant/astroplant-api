use astroplant_mqtt::RawMeasurement;
use axum::extract::ws::{Message as WsMessage, WebSocket};
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use futures_channel::mpsc;
use jsonrpsee::core::server::helpers::{BoundedSubscriptions, MethodSink};
use jsonrpsee::core::server::rpc_module::{ConnState, MethodKind};
use jsonrpsee::types::error::ErrorCode;
use jsonrpsee::types::params::Params;
use jsonrpsee::types::request::Request;
use jsonrpsee::ws_server::RandomIntegerIdProvider;
use jsonrpsee::RpcModule;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{broadcast, oneshot};

// Note this implementation uses std::sync Mutex and RwLock. These are more performant than Tokio's
// async counterparts, but should not be held across await points.

mod rpc_impl {
    use jsonrpsee::core::server::rpc_module::PendingSubscription;
    use jsonrpsee::proc_macros::rpc;
    use jsonrpsee::types::error::ErrorObject;
    use tokio::sync::broadcast;
    #[rpc(server)]
    pub trait Rpc {
        #[subscription(name = "subscribe_raw_measurements", item = String)]
        fn sub(&self, kit_serial: String);
    }

    pub(crate) struct RpcServerImpl<F> {
        pub(crate) raw_measurement_listeners:
            std::sync::Arc<std::sync::RwLock<crate::RawMeasurementListeners>>,
        pub(crate) raw_measurement_cache:
            std::sync::Arc<std::sync::Mutex<crate::RawMeasurementCache>>,
        pub(crate) auth_check: F,
    }

    #[async_trait::async_trait]
    impl<F, Fut> RpcServer for RpcServerImpl<F>
    where
        F: Fn(String) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = bool> + Send + 'static,
    {
        fn sub(&self, pending: PendingSubscription, kit_serial: String) {
            let auth_check_fut = (self.auth_check)(kit_serial.clone());
            let raw_measurement_listeners = self.raw_measurement_listeners.clone();
            let raw_measurement_cache = self.raw_measurement_cache.clone();

            tokio::spawn(async move {
                if !auth_check_fut.await {
                    pending.reject(ErrorObject::borrowed(
                        1,
                        &"you are not authorized to subscribe to this kit",
                        None,
                    ));
                    return;
                }

                let mut sink = pending.accept().unwrap();

                // Dump all cached measurements on the sink.
                match raw_measurement_cache.lock().unwrap().get(&kit_serial) {
                    Some(measurements) => measurements.values().for_each(|(_, measurement)| {
                        let _ = sink.send(measurement);
                    }),
                    None => {}
                }

                let receiver = raw_measurement_listeners
                    .read()
                    .unwrap()
                    .get(&kit_serial)
                    .map(|sender| sender.subscribe());

                let mut receiver = match receiver {
                    Some(receiver) => receiver,
                    None => {
                        let (tx, rx) = broadcast::channel(8);
                        raw_measurement_listeners
                            .write()
                            .unwrap()
                            .insert(kit_serial.clone(), tx);
                        rx
                    }
                };

                // We periodically check if the subscription was closed. We are not notified
                // automatically of subscription closure, and may not notice if the kit is not
                // sending measurements.
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));

                loop {
                    tokio::select! {
                        Ok(raw_measurement) = receiver.recv() => {
                            if let Err(_) = sink.send(&raw_measurement) {
                                // Channel closed
                                break;
                            }
                            interval.reset();
                        }
                        _ = interval.tick() => {
                            if sink.is_closed() {
                                break;
                            }
                        }
                    }
                }

                // The subscription was closed. Check if we are the last receiver of measurements
                // for this kit.
                drop(receiver);
                let cnt = raw_measurement_listeners
                    .read()
                    .unwrap()
                    .get(&kit_serial)
                    .expect("invariant")
                    .receiver_count();
                tracing::trace!(
                    "raw measurement subscription for {} was dropped -- there are {} subscribers left",
                    kit_serial,
                    cnt
                );
                if cnt == 0 {
                    raw_measurement_listeners
                        .write()
                        .unwrap()
                        .remove(&kit_serial);
                    tracing::debug!("raw measurement broadcast for {} was dropped", kit_serial);
                }
            });
        }
    }
}

pub fn create() -> (Publisher, SocketHandler) {
    let publisher = Publisher {
        raw_measurement_listeners: Default::default(),
        raw_measurement_cache: Default::default(),
    };

    let socket_handler = SocketHandler {
        id_provider: RandomIntegerIdProvider,
        raw_measurement_listeners: publisher.raw_measurement_listeners.clone(),
        raw_measurement_cache: publisher.raw_measurement_cache.clone(),
        next_connection_id: Default::default(),
    };

    let raw_measurement_cache = publisher.raw_measurement_cache.clone();

    // Spawn a task to flush the raw measurement cache every so often.
    tokio::spawn(keep_raw_measurement_cache_clean(raw_measurement_cache));

    (publisher, socket_handler)
}

async fn keep_raw_measurement_cache_clean(
    raw_measurement_cache: Arc<std::sync::Mutex<RawMeasurementCache>>,
) {
    const CHECK_INTERVAL: Duration = Duration::from_secs(5 * 60);
    const RETENTION_PERIOD: Duration = Duration::from_secs(30 * 60);

    let mut interval = tokio::time::interval(CHECK_INTERVAL);
    loop {
        interval.tick().await;
        let now = Instant::now();
        let mut raw_measurement_cache = raw_measurement_cache.lock().unwrap();
        raw_measurement_cache.retain(|_, for_kit| {
            for_kit.retain(|_, (added, _)| now.duration_since(*added) < RETENTION_PERIOD);
            for_kit.len() > 0
        });
    }
}

// Raw measurements are broadcast per kit serial.
type RawMeasurementListeners = HashMap<String, broadcast::Sender<RawMeasurement>>;
type RawMeasurementCache = HashMap<String, HashMap<(i32, i32), (Instant, RawMeasurement)>>;

#[derive(Clone)]
pub struct Publisher {
    /// Holds a raw measurement broadcast handle for each kit serial.
    raw_measurement_listeners: Arc<std::sync::RwLock<RawMeasurementListeners>>,
    /// Holds the newest raw measurement (in terms of arrival time) for each kit serial and
    /// (peripheral, quantity type) tuple.
    raw_measurement_cache: Arc<std::sync::Mutex<RawMeasurementCache>>,
}

impl Publisher {
    pub async fn publish_raw_measurement(&self, raw_measurement: RawMeasurement) {
        self.raw_measurement_cache
            .lock()
            .unwrap()
            .entry(raw_measurement.kit_serial.clone())
            .or_default()
            .insert(
                (raw_measurement.peripheral, raw_measurement.quantity_type),
                (Instant::now(), raw_measurement.clone()),
            );

        let listeners = self.raw_measurement_listeners.read().unwrap();
        if let Some(sender) = listeners.get(&raw_measurement.kit_serial) {
            // Returns an error if all receivers are dropped, the last-dropped receiver will handle
            // deregistering the sender. If we were to deregister here, a memory leak could occur
            // if a kit never sends measurements after the last receiver is dropped.
            let _ = sender.send(raw_measurement);
        }
    }
}

struct SocketState<'a, F> {
    connection_id: usize,
    id_provider: &'a RandomIntegerIdProvider,
    bounded_subscriptions: BoundedSubscriptions,
    rpc_module: RpcModule<rpc_impl::RpcServerImpl<F>>,
    method_sink: MethodSink,
}

#[derive(Clone)]
pub struct SocketHandler {
    id_provider: RandomIntegerIdProvider,
    raw_measurement_listeners: Arc<std::sync::RwLock<RawMeasurementListeners>>,
    raw_measurement_cache: Arc<std::sync::Mutex<RawMeasurementCache>>,
    next_connection_id: Arc<std::sync::atomic::AtomicUsize>,
}

impl SocketHandler {
    /// Hands off a websocket to the socket handler, including a closure that can be called to
    /// check whether the websocket is allowed to subscribe to raw measurements of a specific kit.
    pub async fn handle<F, Fut>(&self, socket: WebSocket, auth_check: F)
    where
        F: Fn(String) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = bool> + Send + 'static,
    {
        use crate::rpc_impl::RpcServer;

        let connection_id = self
            .next_connection_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let server = rpc_impl::RpcServerImpl {
            raw_measurement_listeners: self.raw_measurement_listeners.clone(),
            raw_measurement_cache: self.raw_measurement_cache.clone(),
            auth_check,
        };

        let (mut sink, stream) = socket.split();
        let (tx, rx) = mpsc::unbounded();
        let (close_tx, close_rx) = oneshot::channel();

        // Spawn a task proxying RPC responses to the WebSocket sink.
        tokio::spawn(async move {
            send_all(&mut sink, rx, close_rx).await;
            tracing::debug!(
                "The sink was closed for WebSocket connection {}",
                connection_id
            );
        });

        let state = SocketState {
            connection_id,
            id_provider: &self.id_provider,
            bounded_subscriptions: BoundedSubscriptions::new(8),
            rpc_module: server.into_rpc(),
            method_sink: MethodSink::new(tx),
        };

        let mut stream = Box::pin(stream);

        while let Some(Ok(ws_msg)) = stream.next().await {
            match ws_msg {
                WsMessage::Text(msg) => self.handle_ws_message(&state, &msg).await,
                _ => {}
            }
        }

        state.bounded_subscriptions.close();
        tracing::debug!(
            "We stopped listening to WebSocket connection {}",
            connection_id
        );

        // Notify the sink proxying task that the WebSocket was closed.
        let _ = close_tx.send(());
    }

    async fn handle_ws_message<F, Fut>(&self, state: &SocketState<'_, F>, message: &str)
    where
        F: Fn(String) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = bool> + Send + 'static,
    {
        if let Ok(req) = serde_json::from_str::<Request>(message) {
            tracing::event!(
                tracing::Level::DEBUG,
                "recv method call {}",
                method = req.method
            );

            let id = req.id.clone();
            let params = Params::new(req.params.map(|params| params.get()));

            match state.rpc_module.method(&req.method) {
                None => {
                    state
                        .method_sink
                        .send_error(req.id, ErrorCode::MethodNotFound.into());
                }
                Some(method) => match method.inner() {
                    MethodKind::Subscription(callback) => {
                        if let Some(cn) = state.bounded_subscriptions.acquire() {
                            let conn_state = ConnState {
                                conn_id: state.connection_id,
                                close_notify: cn,
                                id_provider: state.id_provider,
                            };
                            callback(id, params, state.method_sink.clone(), conn_state);
                        }
                    }
                    _ => {}
                },
            }
        }
    }
}

/// Sink all messages produced by the RPC module to the websocket. Stop when the websocket is
/// closed, or when `close_rx` is signalled.
///
/// This adds a heartbeat to the websocket, sending a ping whenever we haven't sent any other
/// message for a few minutes. We do this as we'd like to keep connections open even when they're
/// idle: users might be changing their kit configurations, with no measurements sent for a long
/// time. When new measurements come in, the user should receive them immediately.
async fn send_all<S>(
    ws_sink: &mut S,
    mut rpc_rx: mpsc::UnboundedReceiver<String>,
    mut close_rx: oneshot::Receiver<()>,
) where
    S: futures::Sink<WsMessage> + Unpin,
{
    const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(4 * 60);

    let mut heartbeat = tokio::time::interval(HEARTBEAT_INTERVAL);
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            Some(msg) = rpc_rx.next() => {
                if ws_sink.send(WsMessage::Text(msg)).await.is_err() {
                    break;
                }
                heartbeat.reset();
            }
            _ = heartbeat.tick() => {
                // WebSocket heartbeat when we haven't sent any data for a while.
                // Generate some data we expect to receive back from the client.
                let time = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or(Duration::from_secs(42));

                if ws_sink
                    .send(WsMessage::Ping(Vec::from(time.as_millis().to_ne_bytes())))
                    .await
                    .is_err() {
                    break;
                };
            }
            _ = &mut close_rx => {
                break;
            }
        }
    }

    let _ = ws_sink.close().await;
}
