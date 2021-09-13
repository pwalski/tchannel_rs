use crate::channel::SharedSubChannels;
use crate::connection::{Config, FrameInput, FrameSenders};

use crate::errors::{ConnectionError, TChannelError};
use crate::frames::headers::InitHeaderKey;
use crate::frames::payloads::Init;
use crate::frames::payloads::{Codec, ErrorMsg, PROTOCOL_VERSION};
use crate::frames::{TFrame, TFrameId, TFrameIdCodec, Type};
use bytes::BytesMut;
use futures::{self, Future};
use futures::{future, StreamExt};
use futures::{SinkExt, TryFutureExt};
use log::debug;
use std::array::IntoIter;
use std::collections::HashMap;
use std::fmt::Debug;
use std::iter::FromIterator;
use std::sync::Arc;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};


use tokio_util::codec::{FramedRead, FramedWrite};

#[derive(Debug)]
pub struct Server {
    subchannels: SharedSubChannels,
    frame_senders: Arc<FrameSenders>,
    buffer_size: usize,
}

type TFramedRead = FramedRead<OwnedReadHalf, TFrameIdCodec>;
type TFramedWrite = FramedWrite<OwnedWriteHalf, TFrameIdCodec>;

impl Server {
    pub fn new(subchannels: SharedSubChannels, buffer_size: usize) -> Server {
        Server {
            subchannels,
            frame_senders: Arc::new(FrameSenders::new(buffer_size)),
            buffer_size,
        }
    }

    pub async fn run(
        config: Arc<Config>,
        subchannels: SharedSubChannels,
    ) -> Result<(), ConnectionError> {
        debug!("Starting server on {}", config.server_address);
        let listener = TcpListener::bind(config.server_address).await?;
        loop {
            let (stream, addr) = listener.accept().await?;
            debug!("Handling incoming connection from {}", addr);
            let subchannels = subchannels.clone();
            let mut server = Server::new(subchannels, config.frame_buffer_size);
            tokio::spawn(async move {
                server
                    .handle_connection(stream)
                    .map_err(|err| debug!("Connection error: {}", err))
                    .await
            });
        }
    }

    async fn handle_connection(&mut self, stream: TcpStream) -> Result<(), ConnectionError> {
        let (read, write) = stream.into_split();
        let mut framed_read = FramedRead::new(read, TFrameIdCodec {});
        let mut framed_write = FramedWrite::new(write, TFrameIdCodec {});

        self.handle_init_handshake(&mut framed_read, &mut framed_write)
            .await?;

        let framed_write = Arc::new(framed_write);
        framed_read
            .filter_map(Self::skip_if_err)
            .then(|frame| self.frame_senders.send(frame))
            .filter_map(Self::skip_if_err) //TODO return error msg on ConnectionError::SendError
            .filter_map(future::ready)
            .then(|msg_frames| self.handle_msg_frames(msg_frames, framed_write.clone()))
            .for_each(Self::log_if_err)
            .await;
        Ok(())
    }

    async fn handle_init_handshake(
        &self,
        framed_read: &mut TFramedRead,
        framed_write: &mut TFramedWrite,
    ) -> Result<(), ConnectionError> {
        match framed_read.next().await {
            Some(Ok(mut frame_id)) => {
                Self::check_init_req(&mut frame_id.frame).await?;
                Ok(Self::send_init_res(framed_write, *frame_id.id()).await?)
            }
            Some(Err(err)) => Err(ConnectionError::FrameError(err)),
            None => Err(ConnectionError::Error(
                "Received no Init request.".to_string(),
            )),
        }
    }

    async fn check_init_req(frame: &mut TFrame) -> Result<(), ConnectionError> {
        match frame.frame_type() {
            Type::InitRequest => {
                let init = Init::decode(frame.payload_mut())?;
                debug!("Received Init response: {:?}", init);
                match *init.version() {
                    PROTOCOL_VERSION => Ok(()),
                    other_version => Err(ConnectionError::Error(format!(
                        "Unsupported protocol version: {} ",
                        other_version
                    ))),
                }
            }
            Type::Error => {
                let error = ErrorMsg::decode(frame.payload_mut())?;
                Err(ConnectionError::MessageError(error))
            }
            other_type => Err(ConnectionError::UnexpectedResponseError(*other_type)),
        }
    }

    async fn send_init_res(
        framed_write: &mut TFramedWrite,
        id: u32,
    ) -> Result<(), ConnectionError> {
        //TODO properly handle Init headers
        let headers = HashMap::from_iter(IntoIter::new([(
            InitHeaderKey::TChannelLanguage.to_string(),
            "rust".to_string(),
        )]));
        let init = Init::new(PROTOCOL_VERSION, headers);
        let mut bytes = BytesMut::new();
        init.encode(&mut bytes)?;
        let init_frame_id = TFrameId::new(id, TFrame::new(Type::InitResponse, bytes.freeze()));
        Ok(framed_write.send(init_frame_id).await?)
    }

    async fn handle_msg_frames(
        &self,
        _frame_input: FrameInput,
        _framed_write: Arc<TFramedWrite>,
    ) -> Result<(), TChannelError> {
        // let (response_code, args, arg_scheme) =
        //     Defragmenter::new(frame_input).read_response().await?;
        //
        // Ok(())
        todo!()
    }

    fn skip_if_err<T, Err: Debug>(result: Result<T, Err>) -> impl Future<Output = Option<T>> {
        match result {
            Ok(value) => future::ready(Some(value)),
            Err(err) => {
                error!("Failure: {:?}", err);
                future::ready(None)
            }
        }
    }

    //TODO ugly. async for sake of Stream#for_each.
    async fn log_if_err(res: Result<(), TChannelError>) {
        if let Some(err) = res.err() {
            error!("Request handling failure: {:?}", err);
        }
    }
}
