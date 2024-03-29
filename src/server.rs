use crate::channel::{SharedSubChannels, TResult};
use crate::config::Config;
use crate::connection::{ConnectionResult, FrameInput, FrameSender, FramesDispatcher};
use crate::defragmentation::RequestDefragmenter;
use crate::errors::{ConnectionError, TChannelError};
use crate::fragmentation::ResponseFragmenter;
use crate::frames::headers::InitHeaderKey;
use crate::frames::payloads::{Codec, CodecResult, ErrorCode, ErrorMsg, PROTOCOL_VERSION};
use crate::frames::payloads::{Init, Tracing};
use crate::frames::{TFrame, TFrameId, TFrameIdCodec, Type};
use crate::SubChannel;
use futures::TryFutureExt;
use futures::{self, SinkExt, StreamExt, TryStreamExt};
use log::debug;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;
use tokio_util::codec::{FramedRead, FramedWrite};

#[derive(Debug)]
pub struct Server {
    subchannels: SharedSubChannels,
    frame_dispatchers: Arc<FramesDispatcher>,
    buffer_size: usize,
    server_tasks: usize,
}

type TFramedRead = FramedRead<OwnedReadHalf, TFrameIdCodec>;
type TFramedWrite = FramedWrite<OwnedWriteHalf, TFrameIdCodec>;

impl Server {
    pub fn new(subchannels: SharedSubChannels, buffer_size: usize, server_tasks: usize) -> Server {
        Server {
            subchannels,
            frame_dispatchers: Arc::new(FramesDispatcher::new(buffer_size)),
            buffer_size,
            server_tasks,
        }
    }

    pub async fn run(config: Arc<Config>, subchannels: SharedSubChannels) -> ConnectionResult<()> {
        debug!("Starting server on {}", config.server_address);
        let listener = TcpListener::bind(config.server_address).await?;
        loop {
            let (stream, addr) = listener.accept().await?;
            debug!("Handling incoming connection from {}", addr);
            let subchannels = subchannels.clone();
            let mut server =
                Server::new(subchannels, config.frame_buffer_size, config.server_tasks);
            tokio::spawn(async move {
                if let Err(err) = server.handle_connection(stream).await {
                    error!("Connection error: {}", err);
                }
            });
        }
    }

    async fn handle_connection(&mut self, stream: TcpStream) -> TResult<()> {
        let (read, write) = stream.into_split();
        let mut framed_read = FramedRead::new(read, TFrameIdCodec {});
        let mut framed_write = FramedWrite::new(write, TFrameIdCodec {});
        self.handle_init_handshake(&mut framed_read, &mut framed_write)
            .await?;

        let (sender, receiver) = mpsc::channel::<TFrameId>(self.buffer_size);
        FrameSender::spawn(framed_write, receiver, self.buffer_size); //TODO should use same value?
        let handler = FrameHandler::new(
            self.subchannels.clone(),
            self.frame_dispatchers.clone(),
            sender,
        );

        framed_read
            .map_err(TChannelError::CodecError)
            .try_filter_map(|frame| self.dispatch_frame(frame))
            .try_for_each_concurrent(self.server_tasks, |(id, frames)| handler.handle(id, frames))
            .await
        //TODO send fatal protocol error when it fails
    }

    async fn dispatch_frame(&self, frame: TFrameId) -> TResult<Option<(u32, FrameInput)>> {
        let id = *frame.id();
        self.frame_dispatchers
            .dispatch(frame)
            .await
            .map(|receiver_option| receiver_option.map(|receiver| (id, receiver)))
            //TODO Figure out tracing.
            .map_err(|err| {
                debug!("Err while dispatching frame: {}", err.to_string());
                TChannelError::from(ConnectionError::from((id, err, Tracing::default())))
            })
    }

    async fn handle_init_handshake(
        &self,
        framed_read: &mut TFramedRead,
        framed_write: &mut TFramedWrite,
    ) -> TResult<()> {
        match framed_read.next().await {
            Some(Ok(mut frame_id)) => {
                Self::check_init_req(&mut frame_id).await?;
                Self::send_init_res(framed_write, *frame_id.id()).await?;
                Ok(())
            }
            Some(Err(err)) => Err(TChannelError::CodecError(err)),
            None => Err(TChannelError::Error(
                "Received no Init request.".to_string(),
            )),
        }
    }

    async fn check_init_req(frame_id: &mut TFrameId) -> ConnectionResult<()> {
        let frame = frame_id.frame_mut();
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
                Err(ConnectionError::MessageErrorId(error, *frame_id.id()))
            }
            other_type => Err(ConnectionError::UnexpectedResponseError(*other_type)),
        }
    }

    async fn send_init_res(framed_write: &mut TFramedWrite, id: u32) -> ConnectionResult<()> {
        //TODO properly handle Init headers
        let headers = HashMap::from_iter(IntoIterator::into_iter([(
            InitHeaderKey::TChannelLanguage.to_string(),
            "rust".to_string(),
        )]));
        let init = Init::new(PROTOCOL_VERSION, headers);
        let init_frame_id =
            TFrameId::new(id, TFrame::new(Type::InitResponse, init.encode_bytes()?));
        Ok(framed_write.send(init_frame_id).await?)
    }
}

#[derive(Debug, new)]
struct FrameHandler {
    subchannels: SharedSubChannels,
    frame_dispatchers: Arc<FramesDispatcher>,
    sender: Sender<TFrameId>,
}

impl FrameHandler {
    pub async fn handle(&self, id: u32, frame_input: FrameInput) -> TResult<()> {
        debug!("Handling message (id {})", &id);
        self.handle_input(id, frame_input)
            .and_then(|frames| self.send_frames(frames))
            .await
    }

    async fn handle_input(&self, id: u32, frame_input: FrameInput) -> TResult<Vec<TFrameId>> {
        let (request_fields, message_args) =
            RequestDefragmenter::new(frame_input).read_request().await?;
        self.frame_dispatchers.deregister(&id).await;
        let subchannel = self.get_subchannel(request_fields.service()).await?;
        let (response_code, response_args) = subchannel.handle(message_args).await?;
        match ResponseFragmenter::new(request_fields.service(), response_code, response_args)
            .create_frames()
        {
            Ok(frames) => Ok(frames.map(|f| TFrameId::new(id, f)).collect().await),
            Err(TChannelError::ConnectionError(err)) => Ok(vec![self.to_error_frame(
                ErrorCode::NetworkError,
                err.to_string(),
                id,
            )?]),
            Err(TChannelError::Error(msg)) => Ok(vec![self.to_error_frame(
                ErrorCode::UnexpectedError,
                msg,
                id,
            )?]),
            Err(err @ TChannelError::ConnectionPoolError(_)) => Err(err),
            Err(err) => Ok(vec![self.to_error_frame(
                ErrorCode::UnexpectedError,
                err.to_string(),
                id,
            )?]),
        }
    }

    async fn get_subchannel<STR: AsRef<str>>(&self, service: STR) -> TResult<Arc<SubChannel>> {
        let subchannels = self.subchannels.read().await;
        subchannels.get(service.as_ref()).cloned().ok_or_else(|| {
            TChannelError::Error(format!("Failed to find subchannel '{}'", service.as_ref()))
        })
    }

    async fn send_frames(&self, frames: Vec<TFrameId>) -> TResult<()> {
        for frame in frames {
            self.sender.send(frame).await?
        }
        Ok(())
    }

    fn to_error_frame(&self, code: ErrorCode, msg: String, id: u32) -> CodecResult<TFrameId> {
        //TODO add meaningful tracing
        let tracing = Tracing::default();
        let err = ErrorMsg::new(code, tracing, msg);
        let frame = TFrame::new(Type::Error, err.encode_bytes()?);
        Ok(TFrameId::new(id, frame))
    }
}
