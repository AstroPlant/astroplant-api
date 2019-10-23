use log::{trace, debug};

use super::{astroplant_capnp, Error};

use capnp::serialize_packed;
use futures::channel::oneshot;
use futures::future::{BoxFuture, FutureExt};
use ratelimit_meter::{algorithms::NonConformanceExt, KeyedRateLimiter};

#[derive(Debug)]
pub enum ServerRpcRequest {
    Version {
        response: oneshot::Sender<String>,
    },
    GetActiveConfiguration {
        kit_serial: String,
        response: oneshot::Sender<Option<serde_json::Value>>,
    },
}

#[derive(Debug)]
pub struct ServerRpcResponse {
    pub kit_serial: String,
    pub bytes: Vec<u8>,
}

struct ServerRpcResponseBuilder {
    kit_serial: String,
    message_builder: capnp::message::Builder<capnp::message::HeapAllocator>,
}

impl ServerRpcResponseBuilder {
    pub fn new(kit_serial: String, id: u64) -> Self {
        let mut message_builder = capnp::message::Builder::new_default();
        let mut response_builder =
            message_builder.init_root::<astroplant_capnp::server_rpc_response::Builder>();
        response_builder.set_id(id);
        Self {
            kit_serial,
            message_builder,
        }
    }

    pub fn set_error_method_not_found(mut self) -> Self {
        let response_builder = self
            .message_builder
            .get_root::<astroplant_capnp::server_rpc_response::Builder>()
            .expect("could not get root");
        response_builder.init_error().set_method_not_found(());
        self
    }

    pub fn set_error_rate_limit(mut self, millis: u64) -> Self {
        let response_builder = self
            .message_builder
            .get_root::<astroplant_capnp::server_rpc_response::Builder>()
            .expect("could not get root");
        response_builder.init_error().set_rate_limit(millis);
        self
    }

    pub fn set_version(mut self, version: String) -> Self {
        let mut response_builder = self
            .message_builder
            .get_root::<astroplant_capnp::server_rpc_response::Builder>()
            .expect("could not get root");
        response_builder.set_version(&version);
        self
    }

    pub fn set_configuration(mut self, configuration: Option<serde_json::Value>) -> Self {
        let response_builder = self
            .message_builder
            .get_root::<astroplant_capnp::server_rpc_response::Builder>()
            .expect("could not get root");
        match configuration {
            Some(configuration) => {
                response_builder
                    .init_get_active_configuration()
                    .set_configuration(&configuration.to_string());
            }
            None => {
                response_builder
                    .init_get_active_configuration()
                    .set_none(());
            }
        }
        self
    }

    pub fn create(self) -> ServerRpcResponse {
        let mut bytes = Vec::new();
        serialize_packed::write_message(&mut bytes, &self.message_builder).unwrap();

        ServerRpcResponse {
            kit_serial: self.kit_serial,
            bytes,
        }
    }
}

pub type ServerRpcResponder<'a> = BoxFuture<'a, Option<ServerRpcResponse>>;

pub struct ServerRpcHandler {
    rate_limiter: KeyedRateLimiter<String>,
}

impl ServerRpcHandler {
    pub fn new() -> Self {
        const NUM_REQUESTS: u32= 15u32;
        const PER: std::time::Duration = std::time::Duration::from_secs(60);

        let rate_limiter = KeyedRateLimiter::<String>::new(
            std::num::NonZeroU32::new(NUM_REQUESTS).unwrap(),
            PER,
        );
        Self { rate_limiter }
    }

    fn check_rate_limit(&mut self, kit_serial: String, request_id: u64) -> Result<(), Error> {
        debug!("request id {} of kit {} was rate limited", request_id, kit_serial);
        match self.rate_limiter.check(kit_serial.clone()) {
            Ok(_) => Ok(()),
            Err(neg) => {
                let response = ServerRpcResponseBuilder::new(kit_serial.clone(), request_id)
                    .set_error_rate_limit(neg.wait_time().as_millis() as u64)
                    .create();
                Err(Error::ServerRpcError(response))
            }
        }
    }

    pub fn handle_rpc_request(
        &mut self,
        kit_serial: String,
        mut payload: &[u8],
    ) -> Result<(ServerRpcRequest, Option<ServerRpcResponder<'static>>), Error> {
        let message_reader =
            serialize_packed::read_message(&mut payload, capnp::message::ReaderOptions::default())
                .unwrap();
        let rpc_request = message_reader
            .get_root::<astroplant_capnp::server_rpc_request::Reader>()
            .map_err(Error::Capnp)?;
        let id: u64 = rpc_request.get_id();

        self.check_rate_limit(kit_serial.clone(), id)?;

        match rpc_request.which().map_err(|_| {
            let response = ServerRpcResponseBuilder::new(kit_serial.clone(), id)
                .set_error_method_not_found()
                .create();
            Error::ServerRpcError(response)
        })? {
            astroplant_capnp::server_rpc_request::Which::Version(_) => {
                trace!("received server RPC version request");

                let (sender, receiver) = oneshot::channel();
                let request = ServerRpcRequest::Version { response: sender };

                let receiver = receiver.map(move |version| match version {
                    Ok(version) => Some(
                        ServerRpcResponseBuilder::new(kit_serial, id)
                            .set_version(version)
                            .create(),
                    ),
                    Err(_) => None,
                });

                Ok((request, Some(receiver.boxed())))
            }
            astroplant_capnp::server_rpc_request::Which::GetActiveConfiguration(_) => {
                trace!("received server RPC active configuration request");

                let (sender, receiver) = oneshot::channel();
                let request = ServerRpcRequest::GetActiveConfiguration {
                    kit_serial: kit_serial.clone(),
                    response: sender,
                };

                let receiver = receiver.map(move |configuration| match configuration {
                    Ok(configuration) => Some(
                        ServerRpcResponseBuilder::new(kit_serial, id)
                            .set_configuration(configuration)
                            .create(),
                    ),
                    Err(_) => None,
                });

                Ok((request, Some(receiver.boxed())))
            }
        }
    }
}
