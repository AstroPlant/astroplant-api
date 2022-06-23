use capnp::serialize_packed;

use super::{astroplant_capnp, RpcError};

pub struct ServerRpcRequest {
    pub id: u64,
    pub body: ServerRpcRequestBody,
}

pub enum ServerRpcRequestBody {
    Version,
    GetActiveConfiguration,
    GetQuantityTypes,
}

pub enum DecodeError {
    WithRequestId { id: u64, error: capnp::Error },
    WithoutRequestId(capnp::Error),
}

pub fn decode_rpc_request(mut message: &[u8]) -> Result<ServerRpcRequest, DecodeError> {
    let message_reader =
        serialize_packed::read_message(&mut message, capnp::message::ReaderOptions::default())
            .map_err(DecodeError::WithoutRequestId)?;
    let request = message_reader
        .get_root::<astroplant_capnp::server_rpc_request::Reader>()
        .map_err(DecodeError::WithoutRequestId)?;

    let id = request.get_id();

    let body = match request.which().map_err(|err| DecodeError::WithRequestId {
        id,
        error: err.into(),
    })? {
        astroplant_capnp::server_rpc_request::Which::Version(_) => ServerRpcRequestBody::Version,
        astroplant_capnp::server_rpc_request::Which::GetActiveConfiguration(_) => {
            ServerRpcRequestBody::GetActiveConfiguration
        }
        astroplant_capnp::server_rpc_request::Which::GetQuantityTypes(_) => {
            ServerRpcRequestBody::GetQuantityTypes
        }
    };

    Ok(ServerRpcRequest { id, body })
}

#[derive(Debug)]
pub struct ServerRpcResponse {
    pub kit_serial: String,
    pub bytes: Vec<u8>,
}

pub struct ServerRpcResponseBuilder {
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

    pub fn set_from_rpc_error(self, rpc_error: RpcError) -> Self {
        match rpc_error {
            RpcError::Other => self.set_error_other(),
            RpcError::MethodNotFound => self.set_error_method_not_found(),
            RpcError::RateLimit(duration) => self.set_error_rate_limit(duration.as_millis() as u64),
        }
    }

    pub fn set_error_other(mut self) -> Self {
        let response_builder = self
            .message_builder
            .get_root::<astroplant_capnp::server_rpc_response::Builder>()
            .expect("could not get root");
        response_builder.init_error().set_other(());
        self
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

    pub fn set_active_configuration(mut self, configuration: Option<serde_json::Value>) -> Self {
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

    pub fn set_quantity_types(mut self, quantity_types: Vec<serde_json::Value>) -> Self {
        let mut response_builder = self
            .message_builder
            .get_root::<astroplant_capnp::server_rpc_response::Builder>()
            .expect("could not get root");
        response_builder.set_get_quantity_types(&serde_json::to_string(&quantity_types).unwrap());
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
