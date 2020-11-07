use super::{astroplant_capnp, Error};

use capnp::serialize_packed;
use futures::channel::oneshot;
use futures::task::SpawnExt;
use rumqtt::{MqttClient, QoS};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::Mutex;

const KIT_RPC_RESPONSE_BUFFER: usize = super::MQTT_API_MESSAGE_BUFFER;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum KitRpcResponseError {
    TimedOut,
    RpcError,
    MalformedResponse,
    InvalidResponse,
}

pub struct PeripheralCommandResponse {
    pub media_type: String,
    pub data: Vec<u8>,
    pub metadata: serde_json::Value,
}

pub enum PeripheralCommandLockRequest {
    Status,
    Acquire,
    Release,
}

pub type KitRpcResponse<T> = Result<T, KitRpcResponseError>;
pub type KitResponseReceiver<T> = oneshot::Receiver<KitRpcResponse<T>>;

enum KitRpcResponseCallback {
    Version(oneshot::Sender<KitRpcResponse<String>>),
    Uptime(oneshot::Sender<KitRpcResponse<std::time::Duration>>),
    PeripheralCommand(oneshot::Sender<KitRpcResponse<PeripheralCommandResponse>>),
    PeripheralCommandLock(oneshot::Sender<KitRpcResponse<bool>>),
}

impl KitRpcResponseCallback {
    pub fn invoke(self, payload: Vec<u8>) -> Result<(), ()> {
        use astroplant_capnp::kit_rpc_response::Which;
        use KitRpcResponseCallback::*;

        let message_reader = serialize_packed::read_message(
            &mut payload.as_ref(),
            capnp::message::ReaderOptions::default(),
        )
        // `invoke` is only called when the message was successfully deserialized before,
        // so this unwrap is safe.
        .unwrap();

        let rpc_response = message_reader
            .get_root::<astroplant_capnp::kit_rpc_response::Reader>()
            // `invoke` is only called when the message was successfully deserialized before,
            // so this unwrap is safe.
            .unwrap();

        let which_response = rpc_response.which();

        match self {
            Version(callback) => {
                if let Ok(Which::Version(Ok(version))) = which_response {
                    callback.send(Ok(version.to_owned())).map_err(|_| ())
                } else if let Ok(Which::Error(_)) = which_response {
                    callback
                        .send(Err(KitRpcResponseError::RpcError))
                        .map_err(|_| ())
                } else {
                    callback
                        .send(Err(KitRpcResponseError::InvalidResponse))
                        .map_err(|_| ())
                }
            }
            Uptime(callback) => {
                if let Ok(Which::Uptime(uptime)) = which_response {
                    callback
                        .send(Ok(std::time::Duration::from_secs(uptime)))
                        .map_err(|_| ())
                } else if let Ok(Which::Error(_)) = which_response {
                    callback
                        .send(Err(KitRpcResponseError::RpcError))
                        .map_err(|_| ())
                } else {
                    callback
                        .send(Err(KitRpcResponseError::InvalidResponse))
                        .map_err(|_| ())
                }
            }
            PeripheralCommand(callback) => {
                fn process(
                    peripheral_command: astroplant_capnp::kit_rpc_response::peripheral_command::Reader,
                ) -> Result<PeripheralCommandResponse, KitRpcResponseError> {
                    Ok(PeripheralCommandResponse {
                        media_type: peripheral_command
                            .get_media_type()
                            .map_err(|_| KitRpcResponseError::MalformedResponse)?
                            .to_string(),
                        data: peripheral_command
                            .get_data()
                            .map_err(|_| KitRpcResponseError::MalformedResponse)?
                            .to_vec(),
                        metadata: serde_json::from_str(
                            peripheral_command
                                .get_metadata()
                                .map_err(|_| KitRpcResponseError::MalformedResponse)?,
                        )
                        .map_err(|_| KitRpcResponseError::MalformedResponse)?,
                    })
                }

                if let Ok(Which::PeripheralCommand(Ok(peripheral_command))) = which_response {
                    callback.send(process(peripheral_command)).map_err(|_| ())
                } else if let Ok(Which::Error(_)) = which_response {
                    callback
                        .send(Err(KitRpcResponseError::RpcError))
                        .map_err(|_| ())
                } else {
                    callback
                        .send(Err(KitRpcResponseError::InvalidResponse))
                        .map_err(|_| ())
                }
            }
            PeripheralCommandLock(callback) => {
                if let Ok(Which::PeripheralCommandLock(locked)) = which_response {
                    callback.send(Ok(locked)).map_err(|_| ())
                } else if let Ok(Which::Error(_)) = which_response {
                    callback
                        .send(Err(KitRpcResponseError::RpcError))
                        .map_err(|_| ())
                } else {
                    callback
                        .send(Err(KitRpcResponseError::InvalidResponse))
                        .map_err(|_| ())
                }
            }
        }
    }

    pub fn time_out(self) {
        use KitRpcResponseCallback::*;

        match self {
            Version(callback) => {
                let _ = callback.send(Err(KitRpcResponseError::TimedOut));
            }
            Uptime(callback) => {
                let _ = callback.send(Err(KitRpcResponseError::TimedOut));
            }
            PeripheralCommand(callback) => {
                let _ = callback.send(Err(KitRpcResponseError::TimedOut));
            }
            PeripheralCommandLock(callback) => {
                let _ = callback.send(Err(KitRpcResponseError::TimedOut));
            }
        };
    }
}

struct Handle {
    mqtt_client: MqttClient,
    next_id: u64,
    callbacks: HashMap<u64, KitRpcResponseCallback>,
    timeouts: VecDeque<(u64, std::time::Instant)>,
}

impl Handle {
    fn get_next_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Insert the response callback, and get the id to be used for request.
    pub fn insert_callback(&mut self, callback: KitRpcResponseCallback) -> u64 {
        let id = self.get_next_id();
        self.callbacks.insert(id, callback);
        self.timeouts.push_back((id, std::time::Instant::now()));
        tracing::trace!("created kit RPC callback with id: {}", id);
        id
    }

    /// Expire old callbacks.
    pub fn cleanup(&mut self) {
        let now = std::time::Instant::now();
        while let Some(&(idx, instant)) = self.timeouts.get(0) {
            if now.duration_since(instant).as_secs() >= 60 {
                self.timeouts.pop_front();
                if let Some(callback) = self.callbacks.remove(&idx) {
                    callback.time_out();
                }
            } else {
                break;
            }
        }
    }
}

struct KitRpcRequest {
    kit_serial: String,
    bytes: Vec<u8>,
}

struct KitRpcRequestBuilder {
    kit_serial: String,
    message_builder: capnp::message::Builder<capnp::message::HeapAllocator>,
}

impl KitRpcRequestBuilder {
    pub fn new(kit_serial: String, id: u64) -> Self {
        let mut message_builder = capnp::message::Builder::new_default();
        let mut request_builder =
            message_builder.init_root::<astroplant_capnp::kit_rpc_request::Builder>();
        request_builder.set_id(id);
        Self {
            kit_serial,
            message_builder,
        }
    }

    pub fn version(mut self) -> Self {
        let mut request_builder = self
            .message_builder
            .get_root::<astroplant_capnp::kit_rpc_request::Builder>()
            .expect("could not get root");
        request_builder.set_version(());
        self
    }

    pub fn uptime(mut self) -> Self {
        let mut request_builder = self
            .message_builder
            .get_root::<astroplant_capnp::kit_rpc_request::Builder>()
            .expect("could not get root");
        request_builder.set_uptime(());
        self
    }

    pub fn peripheral_command(mut self, peripheral: String, command: serde_json::Value) -> Self {
        let request_builder = self
            .message_builder
            .get_root::<astroplant_capnp::kit_rpc_request::Builder>()
            .expect("could not get root");
        let mut command_builder = request_builder.init_peripheral_command();
        command_builder.set_peripheral(&peripheral);
        command_builder.set_command(&serde_json::to_string(&command).unwrap());
        self
    }

    pub fn peripheral_command_lock(mut self, peripheral: String, request: PeripheralCommandLockRequest) -> Self {
        use PeripheralCommandLockRequest::*;

        let request_builder = self
            .message_builder
            .get_root::<astroplant_capnp::kit_rpc_request::Builder>()
            .expect("could not get root");
        let mut lock_builder = request_builder.init_peripheral_command_lock();
        lock_builder.set_peripheral(&peripheral);
        match request {
            Status => lock_builder.set_status(()),
            Acquire => lock_builder.set_acquire(()),
            Release => lock_builder.set_release(()),
        }
        self
    }

    pub fn create(self) -> KitRpcRequest {
        let mut bytes = Vec::new();
        serialize_packed::write_message(&mut bytes, &self.message_builder).unwrap();

        KitRpcRequest {
            kit_serial: self.kit_serial,
            bytes,
        }
    }
}

/// A handle to a kit's RPC.
#[derive(Clone)]
pub struct KitRpc {
    kit_serial: String,
    handle: Arc<Mutex<Handle>>,
}

impl KitRpc {
    fn send(rpc_request: KitRpcRequest, mqtt_client: &mut MqttClient) {
        mqtt_client
            .publish(
                format!("kit/{}/kit-rpc/request", rpc_request.kit_serial),
                QoS::AtLeastOnce,
                false,
                rpc_request.bytes,
            )
            .expect("could not publish kit RPC request to MQTT");
    }

    pub fn version(&self) -> KitResponseReceiver<String> {
        let (sender, receiver) = oneshot::channel();

        let mut handle = self.handle.lock().unwrap();
        let id = handle.insert_callback(KitRpcResponseCallback::Version(sender));

        let request = KitRpcRequestBuilder::new(self.kit_serial.clone(), id)
            .version()
            .create();
        Self::send(request, &mut handle.mqtt_client);

        receiver
    }

    pub fn uptime(&self) -> KitResponseReceiver<std::time::Duration> {
        let (sender, receiver) = oneshot::channel();

        let mut handle = self.handle.lock().unwrap();
        let id = handle.insert_callback(KitRpcResponseCallback::Uptime(sender));

        let request = KitRpcRequestBuilder::new(self.kit_serial.clone(), id)
            .uptime()
            .create();
        Self::send(request, &mut handle.mqtt_client);

        receiver
    }

    pub fn peripheral_command(
        &self,
        peripheral: String,
        command: serde_json::Value,
    ) -> KitResponseReceiver<PeripheralCommandResponse> {
        let (sender, receiver) = oneshot::channel();

        let mut handle = self.handle.lock().unwrap();
        let id = handle.insert_callback(KitRpcResponseCallback::PeripheralCommand(sender));

        let request = KitRpcRequestBuilder::new(self.kit_serial.clone(), id)
            .peripheral_command(peripheral, command)
            .create();
        Self::send(request, &mut handle.mqtt_client);

        receiver
    }

    pub fn peripheral_command_lock(
        &self,
        peripheral: String,
        request: PeripheralCommandLockRequest,
    ) -> KitResponseReceiver<bool> {
        let (sender, receiver) = oneshot::channel();

        let mut handle = self.handle.lock().unwrap();
        let id = handle.insert_callback(KitRpcResponseCallback::PeripheralCommandLock(sender));

        let request = KitRpcRequestBuilder::new(self.kit_serial.clone(), id)
            .peripheral_command_lock(peripheral, request)
            .create();
        Self::send(request, &mut handle.mqtt_client);

        receiver
    }
}

/// A handle to kit RPCs.
#[derive(Clone)]
pub struct KitsRpc {
    handle: Arc<Mutex<Handle>>,
}

impl KitsRpc {
    pub fn new(mqtt_client: MqttClient) -> Self {
        Self {
            handle: Arc::new(Mutex::new(Handle {
                mqtt_client,
                next_id: 0,
                callbacks: HashMap::new(),
                timeouts: VecDeque::new(),
            })),
        }
    }

    pub fn kit_rpc(&self, kit_serial: String) -> KitRpc {
        KitRpc {
            kit_serial,
            handle: self.handle.clone(),
        }
    }
}

/// Intermittently cleans old (timed-out) kit RPC response callbacks.
async fn cleanup(handle: Arc<Mutex<Handle>>) {
    loop {
        futures_timer::Delay::new(std::time::Duration::from_secs(30)).await;
        tracing::trace!("Performing kit RPC response handle cleanup");
        let mut handle = handle.lock().unwrap();
        handle.cleanup();
    }
}

/// Handles kit RPC responses. Deserializes the payload and invokes the kit RPC response callback.
async fn handle_response(handle: Arc<Mutex<Handle>>, kit_serial: String, payload: Vec<u8>) {
    let message_reader = match serialize_packed::read_message(
        &mut payload.as_ref(),
        capnp::message::ReaderOptions::default(),
    ) {
        Ok(r) => r,
        Err(_err) => {
            tracing::debug!("Malformed RPC response from kit {}", kit_serial);
            return;
        }
    };
    let rpc_response = match message_reader
        .get_root::<astroplant_capnp::kit_rpc_response::Reader>()
        .map_err(Error::Capnp)
    {
        Ok(r) => r,
        Err(_err) => {
            tracing::debug!("Malformed RPC response from kit {}", kit_serial);
            return;
        }
    };

    let id = rpc_response.get_id();
    let mut handle = handle.lock().unwrap();

    tracing::trace!("received kit RPC response for id: {}", id);

    if let Some(callback) = handle.callbacks.remove(&id) {
        callback
            .invoke(payload)
            .expect("kit RPC response callback went away");
    }
}

pub struct KitsRpcRunner {
    pub kits_rpc: KitsRpc,
    pub mqtt_message_handler: crossbeam_channel::Sender<(String, Vec<u8>)>,
}

pub fn kit_rpc_runner(
    mqtt_client: MqttClient,
    thread_pool: futures::executor::ThreadPool,
) -> KitsRpcRunner {
    let kits_rpc = KitsRpc::new(mqtt_client);
    let (sender, receiver) = crossbeam_channel::bounded(KIT_RPC_RESPONSE_BUFFER);
    thread_pool
        .spawn(cleanup(kits_rpc.handle.clone()))
        .expect("Could not spawn kit RPC response handler cleanup");

    {
        let handle = kits_rpc.handle.clone();
        let thread_pool = thread_pool.clone();
        std::thread::spawn(move || {
            for (kit_serial, payload) in receiver {
                tracing::trace!(
                    "received a message on the kit RPC response channel from {}",
                    kit_serial
                );
                if let Err(err) =
                    thread_pool.spawn(handle_response(handle.clone(), kit_serial, payload))
                {
                    tracing::warn!(
                        "Could not spawn kit RPC response handler onto threadpool: {:?}",
                        err
                    );
                }
            }
        });
    }

    KitsRpcRunner {
        kits_rpc,
        mqtt_message_handler: sender,
    }
}
