//! Implementation of the AstroPlant back-end MQTT client. This abstracts away over the underlying
//! MQTT protocol.
//!
//! The client exposes a kit RPC handle to send RPC requests to kits. The client can be given a
//! server RPC handler (to handle kits' requests to the server RPC). A Tokio task is spawned for
//! each server RPC request.

use async_trait::async_trait;
use capnp::serialize_packed;
use futures::Stream;
use ratelimit_meter::{algorithms::NonConformance, KeyedRateLimiter};
use rumqttc::{AsyncClient, Event, EventLoop, MqttOptions, Packet, Publish};
use std::time::{Duration, Instant};
use std::{collections::HashMap, convert::TryFrom};

mod kit_rpc;
mod server_rpc;
use kit_rpc::{Driver as KitsRpcDriver, ResponseTx as KitsRpcResponseTx};
use server_rpc::{
    ServerRpcRequest, ServerRpcRequestBody, ServerRpcResponse, ServerRpcResponseBuilder,
};

pub use kit_rpc::{DecodeError, KitRpcResponseError, KitsRpc};

#[allow(dead_code)]
mod astroplant_capnp {
    include!(concat!(env!("OUT_DIR"), "/proto/astroplant_capnp.rs"));
}

/// Errors sent as a response of an RPC.
///
/// This can either be an error response by the server to a server RPC request made by a kit, or a
/// response by a kit to a kit RPC request made by the server.
#[derive(thiserror::Error, Debug)]
pub enum RpcError {
    /// An unspecified error occurred. This may indicate internal errors or malformed requests.
    #[error("An unspecified error occurred")]
    Other,
    /// The requested RPC method was not found.
    #[error("The requested RPC method was not found")]
    MethodNotFound,
    /// The request was rated limited. The [duration](Duration) indicates when the next request can
    /// be made.
    #[error("The RPC request was rate limited. Next request can be made in {} milliseconds", .0.as_millis())]
    RateLimit(Duration),
}

/// Errors that can happen.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// A message was seen on an invalid topic.
    #[error("An invalid topic was encountered: {0}")]
    InvalidTopic(String),

    /// A kit sent a malformed message.
    #[error("A malformed message was encountered")]
    MalformedMessage { kit_serial: String },

    /// A kit responded erroneously to an RPC request.
    ///
    /// Erroneous kit RPC responses that can be matched to a specific request are sent as
    /// [KitRpcResponseError] to the specific request.
    #[error("There was an issue with a kit RPC response")]
    KitRpcResponse {
        kit_serial: String,
        #[source]
        error: DecodeError,
    },

    /// A message could not be decoded.
    #[error("An issue occurred when trying to decode the message")]
    DecodingIssue {
        kit_serial: String,
        #[source]
        error: capnp::Error,
    },

    /// An MQTT connection issue occurred.
    #[error("An MQTT connection issue occurred")]
    Mqtt(#[from] rumqttc::ConnectionError),

    /// An MQTT client error occurred.
    #[error("An MQTT client issue occurred")]
    MqttClientError(#[from] rumqttc::ClientError),
}

impl Error {
    fn from_kit_serial_and_capnp(kit_serial: String, error: impl Into<capnp::Error>) -> Self {
        Error::DecodingIssue {
            kit_serial,
            error: error.into(),
        }
    }
}

/// A raw measurement made by a kit.
#[derive(Debug)]
pub struct RawMeasurement {
    pub id: uuid::Uuid,
    pub kit_serial: String,
    pub datetime: u64,
    pub peripheral: i32,
    pub quantity_type: i32,
    pub value: f64,
}

/// An aggregate of raw measurements made by a kit.
#[derive(Debug)]
pub struct AggregateMeasurement {
    pub id: uuid::Uuid,
    pub kit_serial: String,
    pub datetime_start: u64,
    pub datetime_end: u64,
    pub peripheral: i32,
    pub quantity_type: i32,
    pub values: HashMap<String, f64>,
}

/// Media produced by a kit.
#[derive(Debug)]
pub struct Media {
    pub id: uuid::Uuid,
    pub kit_serial: String,
    pub datetime: u64,
    pub peripheral: i32,
    pub name: String,
    pub r#type: String,
    pub data: Vec<u8>,
    pub metadata: serde_json::Value,
}

fn parse_raw_measurement(kit_serial: String, mut payload: &[u8]) -> Result<RawMeasurement, Error> {
    let message_reader =
        serialize_packed::read_message(&mut payload, capnp::message::ReaderOptions::default())
            .map_err(|err| Error::from_kit_serial_and_capnp(kit_serial.clone(), err))?;
    let raw_measurement = message_reader
        .get_root::<astroplant_capnp::raw_measurement::Reader>()
        .map_err(|err| Error::from_kit_serial_and_capnp(kit_serial.clone(), err))?;

    let id = raw_measurement
        .get_id()
        .map_err(|err| Error::from_kit_serial_and_capnp(kit_serial.clone(), err))?;

    let measurement = RawMeasurement {
        id: uuid::Uuid::from_slice(id).map_err(|_| Error::MalformedMessage {
            kit_serial: kit_serial.clone(),
        })?,
        datetime: raw_measurement.get_datetime(),
        peripheral: raw_measurement.get_peripheral(),
        quantity_type: raw_measurement.get_quantity_type(),
        value: raw_measurement.get_value(),
        kit_serial,
    };

    Ok(measurement)
}

fn parse_aggregate_measurement(
    kit_serial: String,
    mut payload: &[u8],
) -> Result<AggregateMeasurement, Error> {
    let message_reader =
        serialize_packed::read_message(&mut payload, capnp::message::ReaderOptions::default())
            .map_err(|err| Error::from_kit_serial_and_capnp(kit_serial.clone(), err))?;
    let aggregate_measurement = message_reader
        .get_root::<astroplant_capnp::aggregate_measurement::Reader>()
        .map_err(|err| Error::from_kit_serial_and_capnp(kit_serial.clone(), err))?;

    let id = aggregate_measurement
        .get_id()
        .map_err(|err| Error::from_kit_serial_and_capnp(kit_serial.clone(), err))?;

    let measurement = AggregateMeasurement {
        id: uuid::Uuid::from_slice(id).map_err(|_| Error::MalformedMessage {
            kit_serial: kit_serial.clone(),
        })?,
        datetime_start: aggregate_measurement.get_datetime_start(),
        datetime_end: aggregate_measurement.get_datetime_end(),
        peripheral: aggregate_measurement.get_peripheral(),
        quantity_type: aggregate_measurement.get_quantity_type(),
        values: aggregate_measurement
            .get_values()
            .map_err(|err| Error::from_kit_serial_and_capnp(kit_serial.clone(), err))?
            .into_iter()
            .map(|v| {
                let aggregate_type = v
                    .get_type()
                    .map_err(|err| Error::from_kit_serial_and_capnp(kit_serial.clone(), err))?;
                Ok((aggregate_type.to_owned(), v.get_value()))
            })
            .collect::<Result<_, Error>>()?,
        kit_serial,
    };

    Ok(measurement)
}

fn parse_media(kit_serial: String, mut payload: &[u8]) -> Result<Media, Error> {
    let message_reader =
        serialize_packed::read_message(&mut payload, capnp::message::ReaderOptions::default())
            .map_err(|err| Error::from_kit_serial_and_capnp(kit_serial.clone(), err))?;
    let media = message_reader
        .get_root::<astroplant_capnp::media::Reader>()
        .map_err(|err| Error::from_kit_serial_and_capnp(kit_serial.clone(), err))?;

    let id = media
        .get_id()
        .map_err(|err| Error::from_kit_serial_and_capnp(kit_serial.clone(), err))?;
    let metadata = media
        .get_metadata()
        .map_err(|err| Error::from_kit_serial_and_capnp(kit_serial.clone(), err))?;

    let media = Media {
        id: uuid::Uuid::from_slice(id).map_err(|_| Error::MalformedMessage {
            kit_serial: kit_serial.clone(),
        })?,
        metadata: serde_json::from_str(metadata).map_err(|_| Error::MalformedMessage {
            kit_serial: kit_serial.clone(),
        })?,
        datetime: media.get_datetime(),
        peripheral: media.get_peripheral(),
        name: media
            .get_name()
            .map_err(|err| Error::from_kit_serial_and_capnp(kit_serial.clone(), err))?
            .to_owned(),
        r#type: media
            .get_type()
            .map_err(|err| Error::from_kit_serial_and_capnp(kit_serial.clone(), err))?
            .to_owned(),
        data: media
            .get_data()
            .map_err(|err| Error::from_kit_serial_and_capnp(kit_serial.clone(), err))?
            .to_owned(),
        kit_serial,
    };

    Ok(media)
}

/// A message sent by a kit.
///
/// These messages do not include requests to the server RPC, nor
/// responses to kit RPC requests.
#[derive(Debug)]
pub enum Message {
    RawMeasurement(RawMeasurement),
    AggregateMeasurement(AggregateMeasurement),
    Media(Media),
}

/// A server RPC request handler.
///
/// The handler must respond to each request with a value or an [RpcError] (probably
/// [RpcError::Other]). Rate limiting is handled internally by this library. An implementation
/// should use `#[async_trait]` to allow for the implementation of the async methods.
///
/// # Example
/// ```
/// use async_trait::async_trait;
/// use astroplant_mqtt::{RpcError, ServerRpcHandler};
///
/// struct Handler;
///
/// #[async_trait]
/// impl ServerRpcHandler for Handler {
///     async fn version(&self) -> Result<String, RpcError> {
///         Ok("0.0.1".to_owned())
///     }
///
///     async fn get_active_configuration(
///         &self,
///         kit_serial: String,
///     ) -> Result<Option<serde_json::Value>, RpcError> {
///         Ok(None)
///     }
///
///     async fn get_quantity_types(&self) -> Result<Vec<serde_json::Value>, RpcError> {
///         Ok(vec![])
///     }
/// }
/// ```
#[async_trait]
pub trait ServerRpcHandler {
    async fn version(&self) -> Result<String, RpcError>;
    async fn get_active_configuration(
        &self,
        kit_serial: String,
    ) -> Result<Option<serde_json::Value>, RpcError>;
    async fn get_quantity_types(&self) -> Result<Vec<serde_json::Value>, RpcError>;
}

/// A marker type for when no server RPC handler is given.
///
/// Unconstructable.
pub enum NullHandler {}

#[async_trait]
impl ServerRpcHandler for NullHandler {
    async fn version(&self) -> Result<String, RpcError> {
        unimplemented!()
    }
    async fn get_active_configuration(
        &self,
        _kit_serial: String,
    ) -> Result<Option<serde_json::Value>, RpcError> {
        unimplemented!()
    }
    async fn get_quantity_types(&self) -> Result<Vec<serde_json::Value>, RpcError> {
        unimplemented!()
    }
}

enum TopicKind {
    RawMeasurement,
    AggregateMeasurement,
    Media,
    ServerRpcRequest,
    ServerRpcResponse,
    KitRpcRequest,
    KitRpcResponse,
}

struct Topic {
    kit_serial: String,
    kind: TopicKind,
}

impl TryFrom<&str> for Topic {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let mut topic_parts = value.split("/");
        if topic_parts.next() != Some("kit") {
            return Err(Error::InvalidTopic(value.to_owned()));
        }

        let kit_serial: String = match topic_parts.next() {
            Some(serial) => serial.to_owned(),
            None => return Err(Error::InvalidTopic(value.to_owned())),
        };

        let kind = match (topic_parts.next(), topic_parts.next(), topic_parts.next()) {
            (Some("measurement"), Some("raw"), None) => TopicKind::RawMeasurement,
            (Some("measurement"), Some("aggregate"), None) => TopicKind::AggregateMeasurement,
            (Some("media"), None, None) => TopicKind::Media,
            (Some("server-rpc"), Some("request"), None) => TopicKind::ServerRpcRequest,
            (Some("server-rpc"), Some("response"), None) => TopicKind::ServerRpcResponse,
            (Some("kit-rpc"), Some("request"), None) => TopicKind::KitRpcRequest,
            (Some("kit-rpc"), Some("response"), None) => TopicKind::KitRpcResponse,
            _ => return Err(Error::InvalidTopic(value.to_owned())),
        };

        Ok(Topic { kit_serial, kind })
    }
}

impl TryFrom<String> for Topic {
    type Error = Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_from(value.as_str())
    }
}

/// An MQTT connection handle.
///
/// It must be consumed into a stream, and the stream driven, in order for the underlying protocol
/// to make progress.
pub struct Connection<H> {
    client: AsyncClient,
    event_loop: EventLoop,
    server_rpc_handler: Option<std::sync::Arc<H>>,
    server_rpc_rate_limiter: KeyedRateLimiter<String>,
    kits_rpc_driver: KitsRpcDriver,
    kits_rpc_response_tx: KitsRpcResponseTx,
}

impl<H> Connection<H>
where
    H: ServerRpcHandler + Send + Sync + 'static,
{
    /// Stream the connection contents. This stream must continuously be consumed for the
    /// underlying connection to make progress, including the server and kit RPC. This means the
    /// stream *should not* be used in the same task as the kit RPC, unless you are careful to
    /// continue polling the stream. If the stream isn't polled across a kit RPC await point, a
    /// deadlock occurs.
    pub fn into_stream(self) -> impl Stream<Item = Result<Message, Error>> + Unpin {
        let Self {
            client,
            event_loop,
            server_rpc_handler,
            server_rpc_rate_limiter,
            kits_rpc_driver,
            kits_rpc_response_tx,
        } = self;
        tracing::debug!("MQTT client started");
        tokio::spawn(kits_rpc_driver.drive());

        struct InnerState<H> {
            client: AsyncClient,
            event_loop: EventLoop,
            server_rpc_handler: Option<std::sync::Arc<H>>,
            server_rpc_rate_limiter: KeyedRateLimiter<String>,
            kits_rpc_response_tx: KitsRpcResponseTx,
        }

        async fn step<H>(state: &mut InnerState<H>) -> Result<Option<Message>, Error>
        where
            H: ServerRpcHandler + Send + Sync + 'static,
        {
            let event = state.event_loop.poll().await?;

            match event {
                Event::Incoming(Packet::ConnAck(_)) => {
                    tracing::debug!("MQTT client connected");
                    state
                        .client
                        .subscribe("kit/#", rumqttc::QoS::AtLeastOnce)
                        .await?;
                }
                Event::Incoming(Packet::Publish(publish)) => {
                    tracing::trace!("Received Publish packet");
                    if let Some(message) = handle_publish(
                        &state.client,
                        &state.server_rpc_handler,
                        &mut state.server_rpc_rate_limiter,
                        &state.kits_rpc_response_tx,
                        publish,
                    )
                    .await?
                    {
                        return Ok(Some(message));
                    }
                }
                _ => {}
            }

            Ok(None)
        }

        let stream = futures::stream::unfold(
            InnerState {
                client,
                event_loop,
                server_rpc_handler,
                server_rpc_rate_limiter,
                kits_rpc_response_tx,
            },
            |mut state| async {
                let value = loop {
                    match step(&mut state).await {
                        Ok(None) => {}
                        Ok(Some(value)) => break Ok(value),
                        Err(err) => break Err(err),
                    };
                };

                Some((value, state))
            },
        );

        Box::pin(stream)
    }
}

async fn handle_publish<H>(
    client: &AsyncClient,
    server_rpc_handler: &Option<std::sync::Arc<H>>,
    server_rpc_rate_limiter: &mut KeyedRateLimiter<String>,
    kits_rpc_response_tx: &KitsRpcResponseTx,
    publish: Publish,
) -> Result<Option<Message>, Error>
where
    H: ServerRpcHandler + Send + Sync + 'static,
{
    let topic = Topic::try_from(publish.topic)?;

    match topic.kind {
        TopicKind::RawMeasurement => {
            if let Ok(m) = parse_raw_measurement(topic.kit_serial, &publish.payload) {
                Ok(Some(Message::RawMeasurement(m)))
            } else {
                // Ignore decoding errors
                Ok(None)
            }
        }
        TopicKind::AggregateMeasurement => {
            if let Ok(m) = parse_aggregate_measurement(topic.kit_serial, &publish.payload) {
                Ok(Some(Message::AggregateMeasurement(m)))
            } else {
                // Ignore decoding errors
                Ok(None)
            }
        }
        TopicKind::Media => {
            if let Ok(m) = parse_media(topic.kit_serial, &publish.payload) {
                Ok(Some(Message::Media(m)))
            } else {
                // Ignore decoding errors
                Ok(None)
            }
        }
        TopicKind::ServerRpcRequest => {
            if let Some(server_rpc_handler) = server_rpc_handler {
                handle_server_rpc_request(
                    client,
                    &server_rpc_handler,
                    server_rpc_rate_limiter,
                    topic.kit_serial,
                    &publish.payload,
                )
                .await?;
            }

            Ok(None)
        }
        TopicKind::KitRpcResponse => {
            handle_kit_rpc_response(kits_rpc_response_tx, topic.kit_serial, &publish.payload)
                .await?;
            Ok(None)
        }
        TopicKind::ServerRpcResponse | TopicKind::KitRpcRequest => {
            // Ignored: we send on these topics ourselves
            Ok(None)
        }
    }
}

async fn call_server_rpc_handler<H>(
    server_rpc_handler: std::sync::Arc<H>,
    kit_serial: String,
    request: ServerRpcRequest,
) -> ServerRpcResponse
where
    H: ServerRpcHandler + Send + Sync + 'static,
{
    let response = ServerRpcResponseBuilder::new(kit_serial.clone(), request.id);

    match request.body {
        ServerRpcRequestBody::Version => match server_rpc_handler.version().await {
            Ok(v) => response.set_version(v).create(),
            Err(v) => response.set_from_rpc_error(v).create(),
        },
        ServerRpcRequestBody::GetActiveConfiguration => match server_rpc_handler
            .get_active_configuration(kit_serial)
            .await
        {
            Ok(v) => response.set_active_configuration(v).create(),
            Err(v) => response.set_from_rpc_error(v).create(),
        },
        ServerRpcRequestBody::GetQuantityTypes => {
            match server_rpc_handler.get_quantity_types().await {
                Ok(v) => response.set_quantity_types(v).create(),
                Err(v) => response.set_from_rpc_error(v).create(),
            }
        }
    }
}

async fn handle_kit_rpc_response(
    kits_rpc_response_tx: &KitsRpcResponseTx,
    kit_serial: String,
    payload: &[u8],
) -> Result<(), Error> {
    kits_rpc_response_tx
        .send(kit_serial.clone(), payload.to_owned())
        .await
        .map_err(|error| Error::KitRpcResponse { kit_serial, error })?;

    Ok(())
}

async fn handle_server_rpc_request<H>(
    client: &AsyncClient,
    server_rpc_handler: &std::sync::Arc<H>,
    server_rpc_rate_limiter: &mut KeyedRateLimiter<String>,
    kit_serial: String,
    payload: &[u8],
) -> Result<(), Error>
where
    H: ServerRpcHandler + Send + Sync + 'static,
{
    let request = crate::server_rpc::decode_rpc_request(payload);
    let client = client.clone();
    let handler = server_rpc_handler.clone();

    let rate_limit_wait_time = server_rpc_rate_limiter
        .check(kit_serial.clone())
        .err()
        .map(|neg| neg.wait_time_from(Instant::now()).as_millis() as u64);

    // FIXME:
    // This spawns a task for every RPC request. This can lead to uncontrolled resource consumption
    // when it's busy. There should be a queue for task spawning, such that backpressure can be
    // applied.

    tokio::spawn(async move {
        let response = match request {
            Err(crate::server_rpc::DecodeError::WithRequestId { id, .. }) => {
                let response = ServerRpcResponseBuilder::new(kit_serial.clone(), id)
                    .set_error_method_not_found()
                    .create();
                Some(response)
            }
            Err(crate::server_rpc::DecodeError::WithoutRequestId(_)) => None,
            Ok(request) => {
                if let Some(wait_time) = rate_limit_wait_time {
                    let response = ServerRpcResponseBuilder::new(kit_serial.clone(), request.id)
                        .set_error_rate_limit(wait_time)
                        .create();
                    Some(response)
                } else {
                    Some(call_server_rpc_handler(handler, kit_serial, request).await)
                }
            }
        };

        if let Some(response) = response {
            let _ = client
                .publish(
                    format!("kit/{}/server-rpc/response", response.kit_serial),
                    rumqttc::QoS::AtLeastOnce,
                    false,
                    response.bytes,
                )
                .await;
        }
    });

    Ok(())
}

/// An MQTT connection builder.
pub struct ConnectionBuilder<H> {
    // TODO: allow specifying subscriptions.
    host: String,
    port: u16,
    username: Option<String>,
    password: Option<String>,
    server_rpc_handler: Option<H>,
}

impl ConnectionBuilder<NullHandler> {
    /// Create a connection builder.
    pub fn new<S: Into<String>>(host: S, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
            username: None,
            password: None,
            server_rpc_handler: None,
        }
    }
}

impl<H> ConnectionBuilder<H> {
    /// Specify credentials to use when establishing the connection.
    pub fn with_credentials<S1: Into<String>, S2: Into<String>>(
        self,
        username: S1,
        password: S2,
    ) -> Self {
        Self {
            username: Some(username.into()),
            password: Some(password.into()),
            ..self
        }
    }

    /// Add a server RPC handler to respond to requests made by kits to the server. The handler
    /// should implement the [ServerRpcHandler] trait. If no handler is added, this MQTT client
    /// ignores RPC requests. This allows a different MQTT client to handle requests.
    pub fn with_server_rpc_handler<I>(self, server_rpc_handler: I) -> ConnectionBuilder<I> {
        ConnectionBuilder {
            host: self.host,
            port: self.port,
            username: self.username,
            password: self.password,
            server_rpc_handler: Some(server_rpc_handler),
        }
    }

    /// Create the MQTT client. Returns a connection and a kits RPC handle. The connection must be
    /// driven for the underlying protocol to make progress.
    pub fn create(self) -> (Connection<H>, KitsRpc) {
        let mut options = MqttOptions::new("astroplant-mqtt", self.host, self.port);
        options.set_max_packet_size(
            // Note: capnproto traversal is limited to 64 MiB as well
            64 * 1024 * 1024, // incoming: 64 MiB
            8 * 1024 * 1024,  // outgoing: 8 MiB
        );
        if self.username.is_some() {
            options.set_credentials(self.username.unwrap(), self.password.unwrap());
        }
        options.set_keep_alive(Duration::from_secs(10));
        let (client, event_loop) = AsyncClient::new(options, 32);

        let (kits_rpc, kits_rpc_driver, kits_rpc_response_tx) =
            crate::kit_rpc::create(client.clone());

        let server_rpc_rate_limiter = {
            const NUM_REQUESTS: u32 = 30u32;
            const PER: Duration = Duration::from_secs(60);

            KeyedRateLimiter::<String>::new(std::num::NonZeroU32::new(NUM_REQUESTS).unwrap(), PER)
        };

        let connection = Connection {
            client,
            event_loop,
            server_rpc_handler: self.server_rpc_handler.map(std::sync::Arc::new),
            server_rpc_rate_limiter,
            kits_rpc_driver,
            kits_rpc_response_tx,
        };

        (connection, kits_rpc)
    }
}
