use futures::compat::{Future01CompatExt, Stream01CompatExt};
use futures::{select, Sink, SinkExt, StreamExt};
use jsonrpc_core::futures as futuresOne;
use jsonrpc_core::MetaIoHandler;
use std::sync::Arc;
use warp::ws::{Message, WebSocket};

async fn handle_rpc_msg<S>(socket_sink: &mut S, msg: &str) -> Result<(), ()>
where
    S: Sink<Message> + std::marker::Unpin,
{
    let msg = Message::text(msg);
    if socket_sink.send(msg).await.is_ok() {
        Ok(())
    } else {
        Err(())
    }
}

async fn handle_web_socket_msg<S>(
    socket_sink: &mut S,
    io_handler: &MetaIoHandler<Arc<jsonrpc_pubsub::Session>>,
    context: Arc<jsonrpc_pubsub::Session>,
    msg: &str,
) -> Result<(), ()>
where
    S: Sink<Message> + std::marker::Unpin,
{
    match io_handler.handle_request(msg, context).compat().await {
        Ok(Some(rpc_response)) => handle_rpc_msg(socket_sink, &rpc_response).await,
        Ok(None) => Ok(()),
        Err(_) => Err(()),
    }
}

pub async fn handle_session(
    socket_id: usize,
    web_socket: WebSocket,
    io_handler: MetaIoHandler<Arc<jsonrpc_pubsub::Session>>,
) {
    let (mut socket_sink, socket_stream) = web_socket.split();
    let (rpc_to_socket_sender, rpc_receiver) = futuresOne::sync::mpsc::channel::<String>(64);

    let mut rpc_receiver = rpc_receiver.compat().fuse();
    let mut socket_stream = socket_stream.fuse();
    let context = Arc::new(jsonrpc_pubsub::Session::new(rpc_to_socket_sender));

    loop {
        select! {
            from_rpc_msg = rpc_receiver.next() => {
                if let Some(Ok(from_rpc_msg)) = from_rpc_msg {
                    tracing::trace!("WebSocket {} handling RPC message: {}", socket_id, from_rpc_msg);
                    if handle_rpc_msg(&mut socket_sink, &from_rpc_msg).await.is_err() {
                        tracing::debug!("WebSocket {} encountered error while handling RPC-to-socket message", socket_id);
                        break;
                    }
                } else {
                    tracing::debug!("WebSocket {} RPC terminated or there was an error sending the RPC message to WebSocket", socket_id);
                    break;
                }
            },
            socket_msg = socket_stream.next() => {
                if let Some(Ok(from_socket_msg)) = socket_msg {
                    if let Ok(msg) = from_socket_msg.to_str() {
                        tracing::trace!("WebSocket {} handling socket message: {}", socket_id, msg);
                        if handle_web_socket_msg(&mut socket_sink, &io_handler, context.clone(), &msg).await.is_err() {
                            tracing::debug!("WebSocket {} encountered error while handling WebSocket-to-RPC message", socket_id);
                            break;
                        }
                    }
                } else if let Some(Err(err)) = socket_msg {
                    tracing::debug!("WebSocket {} encountered error on transport: {:?}", socket_id, err);
                    break;
                } else {
                    tracing::debug!("WebSocket {} transport has terminated", socket_id);
                    break;
                }
            }
        }
    }
}
