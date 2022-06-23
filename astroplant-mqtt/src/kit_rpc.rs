use capnp::serialize_packed;
use rumqttc::{AsyncClient, QoS};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot};

use super::{astroplant_capnp, RpcError};

pub enum PeripheralCommandLockRequest {
    Status,
    Acquire,
    Release,
}

enum RequestBody {
    Version,
    Uptime,
    PeripheralCommand {
        peripheral: String,
        command: serde_json::Value,
    },
    PeripheralCommandLock {
        peripheral: String,
        request: PeripheralCommandLockRequest,
    },
}

impl RequestBody {
    fn build(self, request_id: u64) -> Vec<u8> {
        let mut message_builder = capnp::message::Builder::new_default();
        let mut request_builder =
            message_builder.init_root::<astroplant_capnp::kit_rpc_request::Builder>();
        request_builder.set_id(request_id);

        use RequestBody::*;
        match self {
            Version => {
                request_builder.set_version(());
            }
            Uptime => {
                request_builder.set_uptime(());
            }
            PeripheralCommand {
                peripheral,
                command,
            } => {
                let mut builder = request_builder.init_peripheral_command();
                builder.set_peripheral(&peripheral);
                builder.set_command(&serde_json::to_string(&command).unwrap());
            }
            PeripheralCommandLock {
                peripheral,
                request,
            } => {
                let mut builder = request_builder.init_peripheral_command_lock();
                builder.set_peripheral(&peripheral);
                match request {
                    PeripheralCommandLockRequest::Status => builder.set_status(()),
                    PeripheralCommandLockRequest::Acquire => builder.set_acquire(()),
                    PeripheralCommandLockRequest::Release => builder.set_release(()),
                };
            }
        }

        let mut bytes = Vec::new();
        capnp::serialize_packed::write_message(&mut bytes, &message_builder).unwrap();
        bytes
    }
}

struct Request {
    kit_serial: String,
    body: RequestBody,
    response_channel: oneshot::Sender<Result<ResponseBody, DecodeErrorKind>>,
}

struct Response {
    kit_serial: String,
    request_id: u64,
    body: Result<ResponseBody, DecodeErrorKind>,
}

enum ResponseBody {
    Version(String),
    Uptime(std::time::Duration),
    PeripheralCommand(PeripheralCommandResponse),
    PeripheralCommandLock(bool),
    Error(RpcError),
}

#[derive(Debug, thiserror::Error)]
pub enum DecodeErrorKind {
    /// An internal decoding error, e.g., due to lack of resources.
    #[error("An internal decoding error occurred, e.g., due to lack of resoruces")]
    Internal,
    /// A message was malformed and could not be decoded.
    #[error("The message is malformed and cannot not be decoded")]
    Malformed,
}

impl From<capnp::Error> for DecodeErrorKind {
    fn from(error: capnp::Error) -> Self {
        match error.kind {
            capnp::ErrorKind::Failed => DecodeErrorKind::Malformed,
            _ => DecodeErrorKind::Internal,
        }
    }
}

impl From<serde_json::Error> for DecodeErrorKind {
    fn from(_error: serde_json::Error) -> Self {
        DecodeErrorKind::Malformed
    }
}

/// Error that occurs when a kit RPC response message could not be decoded.
#[derive(Debug, thiserror::Error)]
#[error("A decoding error occurred")]
pub struct DecodeError {
    request_id: Option<u64>,
    #[source]
    kind: DecodeErrorKind,
}

impl DecodeError {
    fn with_request_id(request_id: u64, error: impl Into<DecodeErrorKind>) -> Self {
        DecodeError {
            request_id: Some(request_id),
            kind: error.into(),
        }
    }

    fn without_request_id(error: impl Into<DecodeErrorKind>) -> Self {
        DecodeError {
            request_id: None,
            kind: error.into(),
        }
    }
}

fn decode_rpc_error(
    error: astroplant_capnp::rpc_error::Reader,
) -> Result<RpcError, DecodeErrorKind> {
    use astroplant_capnp::rpc_error::Which;

    let err = match error.which().map_err(capnp::Error::from)? {
        Which::Other(()) => RpcError::Other,
        Which::MethodNotFound(()) => RpcError::MethodNotFound,
        Which::RateLimit(millis) => RpcError::RateLimit(std::time::Duration::from_millis(millis)),
    };

    Ok(err)
}

/// Returns the response's request id and the response body.
fn decode_rpc_response(mut message: &[u8]) -> Result<(u64, ResponseBody), DecodeError> {
    let message_reader =
        serialize_packed::read_message(&mut message, capnp::message::ReaderOptions::default())
            .map_err(DecodeError::without_request_id)?;
    let response = message_reader
        .get_root::<astroplant_capnp::kit_rpc_response::Reader>()
        .map_err(DecodeError::without_request_id)?;

    let id = response.get_id();

    use astroplant_capnp::kit_rpc_response::Which;
    let body = match response
        .which()
        .map_err(|err| DecodeError::with_request_id(id, capnp::Error::from(err)))?
    {
        Which::Version(v) => ResponseBody::Version(
            v.map_err(|err| DecodeError::with_request_id(id, err))?
                .to_string(),
        ),
        Which::Uptime(v) => ResponseBody::Uptime(std::time::Duration::from_secs(v)),
        Which::PeripheralCommand(v) => {
            let v = v.map_err(|err| DecodeError::with_request_id(id, err))?;

            let peripheral_command_response = PeripheralCommandResponse {
                media_type: v
                    .get_media_type()
                    .map_err(|err| DecodeError::with_request_id(id, err))?
                    .to_string(),
                data: v
                    .get_data()
                    .map_err(|err| DecodeError::with_request_id(id, err))?
                    .to_vec(),
                metadata: serde_json::from_str(
                    v.get_metadata()
                        .map_err(|err| DecodeError::with_request_id(id, err))?,
                )
                .map_err(|err| DecodeError::with_request_id(id, err))?,
            };

            ResponseBody::PeripheralCommand(peripheral_command_response)
        }
        Which::PeripheralCommandLock(v) => ResponseBody::PeripheralCommandLock(v),
        Which::Error(v) => {
            let v = v.map_err(|err| DecodeError::with_request_id(id, err))?;

            let error = decode_rpc_error(v).map_err(|err| DecodeError::with_request_id(id, err))?;
            ResponseBody::Error(error)
        }
    };

    Ok((id, body))
}

pub(crate) struct ResponseTx(mpsc::Sender<Response>);

impl ResponseTx {
    pub async fn send(&self, kit_serial: String, payload: Vec<u8>) -> Result<(), DecodeError> {
        let (request_id, body) = match decode_rpc_response(&payload) {
            Err(DecodeError {
                request_id: Some(id),
                kind,
            }) => Ok((id, Err(kind))),
            Err(err) => Err(err),
            Ok((request_id, body)) => Ok((request_id, Ok(body))),
        }?;
        let response = Response {
            kit_serial,
            request_id,
            body,
        };
        let _ = self.0.send(response).await;

        Ok(())
    }
}

type SerialAndRequestId = (String, u64);
type Waiter = (
    Instant, // Instant at which waiter was created.
    oneshot::Sender<Result<ResponseBody, DecodeErrorKind>>,
);

pub(crate) struct Driver {
    mqtt: AsyncClient,
    next_id: u64,
    waiters: HashMap<SerialAndRequestId, Waiter>,
    request_rx: mpsc::Receiver<Request>,
    response_rx: mpsc::Receiver<Response>,
}

impl Driver {
    fn new(
        mqtt: AsyncClient,
        request_rx: mpsc::Receiver<Request>,
        response_rx: mpsc::Receiver<Response>,
    ) -> Self {
        Self {
            mqtt,
            next_id: 0,
            waiters: HashMap::new(),
            request_rx,
            response_rx,
        }
    }

    async fn handle_request(&mut self, request: Request) {
        let id = self.next_id;
        self.next_id += 1;

        let _ = self
            .mqtt
            .publish(
                format!("kit/{}/kit-rpc/request", request.kit_serial),
                QoS::AtLeastOnce,
                false,
                request.body.build(id),
            )
            .await;

        self.waiters.insert(
            (request.kit_serial.clone(), id),
            (Instant::now(), request.response_channel),
        );

        tracing::trace!("Sent kit {} RPC request {}", request.kit_serial, id);
    }

    fn cleanup(&mut self) {
        const TIMEOUT: Duration = Duration::from_secs(30);
        let now = Instant::now();
        self.waiters
            .retain(|_, (creation_instant, _)| now.duration_since(*creation_instant) < TIMEOUT);
    }

    pub(crate) async fn drive(mut self) {
        let mut cleanup_interval = tokio::time::interval(Duration::from_secs(1));
        cleanup_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                request = self.request_rx.recv() => {
                    let request = match request {
                        Some(request) => request,
                        // Channel closed.
                        None => break,
                    };
                    self.handle_request(request).await;
                }
                response = self.response_rx.recv() => {
                    let response = match response {
                        Some(response) => response,
                        // Channel closed.
                        None => break,
                    };

                    if let Some((_, tx)) = self.waiters.remove(&(response.kit_serial, response.request_id)) {
                        let _ = tx.send(response.body);
                    }
                }
                _ = cleanup_interval.tick() => {
                    self.cleanup();
                }
            }
        }
    }
}

/// A handle to make kit RPC requests.
#[derive(Clone)]
pub struct KitsRpc {
    request_tx: mpsc::Sender<Request>,
}

/// Errors that can occur in response to a [kit RPC](KitsRpc) request.
///
/// Erroneous kit responses that cannot be matched with a specific kit RPC request, are instead
/// transmitted as a [KitRpcResponse](super::Error::KitRpcResponse) error on the [MQTT connection
/// stream](super::Connection).
#[derive(thiserror::Error, Debug)]
pub enum KitRpcResponseError {
    /// The kit RPC request timed out: no response was received.
    #[error("The kit RPC request timed out: no response was received")]
    TimedOut,
    /// The kit's response to the RPC request could not be decoded.
    #[error("The kit's response to the RPC request could not be decoded")]
    MalformedResponse,
    /// The kit's response to the RPC request was invalid (e.g., a wrong value was returned).
    #[error("The kit's response to the RPC request was invalid")]
    InvalidResponse,
    /// The kit indicated our our request was erroneous.
    #[error("The kit indicated our request was erroneous")]
    RpcError(#[from] RpcError),
}

pub struct PeripheralCommandResponse {
    pub media_type: String,
    pub data: Vec<u8>,
    pub metadata: serde_json::Value,
}

impl KitsRpc {
    pub async fn version(
        &self,
        kit_serial: impl Into<String>,
    ) -> Result<String, KitRpcResponseError> {
        let (tx, rx) = oneshot::channel();
        let _ = self
            .request_tx
            .send(Request {
                kit_serial: kit_serial.into(),
                body: RequestBody::Version,
                response_channel: tx,
            })
            .await;
        match rx.await.map_err(|_| KitRpcResponseError::TimedOut)? {
            Ok(ResponseBody::Version(v)) => Ok(v),
            Ok(ResponseBody::Error(err)) => Err(err.into()),
            Ok(_) => Err(KitRpcResponseError::InvalidResponse),
            Err(_) => Err(KitRpcResponseError::MalformedResponse),
        }
    }

    pub async fn uptime(
        &self,
        kit_serial: impl Into<String>,
    ) -> Result<std::time::Duration, KitRpcResponseError> {
        let (tx, rx) = oneshot::channel();
        let _ = self
            .request_tx
            .send(Request {
                kit_serial: kit_serial.into(),
                body: RequestBody::Uptime,
                response_channel: tx,
            })
            .await;
        match rx.await.map_err(|_| KitRpcResponseError::TimedOut)? {
            Ok(ResponseBody::Uptime(v)) => Ok(v),
            Ok(ResponseBody::Error(err)) => Err(err.into()),
            Ok(_) => Err(KitRpcResponseError::InvalidResponse),
            Err(_) => Err(KitRpcResponseError::MalformedResponse),
        }
    }

    pub async fn peripheral_command(
        &self,
        kit_serial: impl Into<String>,
        peripheral: String,
        command: serde_json::Value,
    ) -> Result<PeripheralCommandResponse, KitRpcResponseError> {
        let (tx, rx) = oneshot::channel();
        let _ = self
            .request_tx
            .send(Request {
                kit_serial: kit_serial.into(),
                body: RequestBody::PeripheralCommand {
                    peripheral,
                    command,
                },
                response_channel: tx,
            })
            .await;
        match rx.await.map_err(|_| KitRpcResponseError::TimedOut)? {
            Ok(ResponseBody::PeripheralCommand(v)) => Ok(v),
            Ok(ResponseBody::Error(err)) => Err(err.into()),
            Ok(_) => Err(KitRpcResponseError::InvalidResponse),
            Err(_) => Err(KitRpcResponseError::MalformedResponse),
        }
    }

    pub async fn peripheral_command_lock(
        &self,
        kit_serial: impl Into<String>,
        peripheral: String,
        request: PeripheralCommandLockRequest,
    ) -> Result<bool, KitRpcResponseError> {
        let (tx, rx) = oneshot::channel();
        let _ = self
            .request_tx
            .send(Request {
                kit_serial: kit_serial.into(),
                body: RequestBody::PeripheralCommandLock {
                    peripheral,
                    request,
                },
                response_channel: tx,
            })
            .await;
        match rx.await.map_err(|_| KitRpcResponseError::TimedOut)? {
            Ok(ResponseBody::PeripheralCommandLock(v)) => Ok(v),
            Ok(ResponseBody::Error(err)) => Err(err.into()),
            Ok(_) => Err(KitRpcResponseError::InvalidResponse),
            Err(_) => Err(KitRpcResponseError::MalformedResponse),
        }
    }
}

pub(crate) fn create(mqtt: AsyncClient) -> (KitsRpc, Driver, ResponseTx) {
    let (request_tx, request_rx) = mpsc::channel(8);
    let (response_tx, response_rx) = mpsc::channel(8);

    let kits_rpc = KitsRpc { request_tx };
    let handler = Driver::new(mqtt, request_rx, response_rx);
    let response_tx = ResponseTx(response_tx);

    (kits_rpc, handler, response_tx)
}
