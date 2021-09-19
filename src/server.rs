use crate::channel::SharedSubChannels;
use crate::connection::{Config, FrameInput, FrameSender, FramesDispatcher};
use crate::defragmentation::RequestDefragmenter;
use crate::errors::{ConnectionError, TChannelError};
use crate::fragmentation::ResponseFragmenter;
use crate::frames::headers::InitHeaderKey;
use crate::frames::payloads::Init;
use crate::frames::payloads::{Codec, ErrorMsg, PROTOCOL_VERSION};
use crate::frames::{TFrame, TFrameId, TFrameIdCodec, TFrameStream, Type};
use crate::SubChannel;
use bytes::BytesMut;
use futures::{self, Future};
use futures::{future, StreamExt};
use futures::{SinkExt, Stream};
use log::{debug, trace};
use std::array::IntoIter;
use std::collections::HashMap;
use std::fmt::Debug;
use std::iter::FromIterator;
use std::pin::Pin;
use std::sync::Arc;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_util::codec::{FramedRead, FramedWrite};

#[derive(Debug)]
pub struct Server {
    subchannels: SharedSubChannels,
    frame_dispatchers: Arc<FramesDispatcher>,
    buffer_size: usize,
}

type TFramedRead = FramedRead<OwnedReadHalf, TFrameIdCodec>;
type TFramedWrite = FramedWrite<OwnedWriteHalf, TFrameIdCodec>;

impl Server {
    pub fn new(subchannels: SharedSubChannels, buffer_size: usize) -> Server {
        Server {
            subchannels,
            frame_dispatchers: Arc::new(FramesDispatcher::new(buffer_size)),
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

            // let (sender, receiver) = mpsc::channel::<TFrameId>(buffer_size);

            let mut server = Server::new(subchannels, config.frame_buffer_size);
            tokio::spawn(async move {
                if let Err(err) = server.handle_connection(stream).await {
                    error!("Connection error: {}", err);
                }
            });
        }
    }

    async fn handle_connection(&mut self, stream: TcpStream) -> Result<(), ConnectionError> {
        let (read, write) = stream.into_split();
        let mut framed_read = FramedRead::new(read, TFrameIdCodec {});
        let mut framed_write = FramedWrite::new(write, TFrameIdCodec {});

        self.handle_init_handshake(&mut framed_read, &mut framed_write)
            .await?;

        let (sender, receiver) = mpsc::channel::<TFrameId>(10);
        FrameSender::spawn(framed_write, receiver, 100);

        framed_read
            .filter_map(Self::skip_if_err)
            .then(|frame| self.dispatch_frame(frame))
            .filter_map(Self::skip_if_err) //TODO return error msg on ConnectionError::SendError
            .filter_map(future::ready) //ugh
            .then(|(id, frame_input)| self.handle_request(id, frame_input))
            .flat_map(|frame_res| match frame_res {
                Err(err) => {
                    error!("Request handling failure: {:?}", err);
                    //TODO return error?
                    Box::pin(futures::stream::iter(Vec::new()))
                }
                Ok(frame_stream) => Box::pin(futures::stream::iter(frame_stream)),
            })
            .for_each(|frame| async {
                match sender.send(frame).await {
                    Ok(()) => (),
                    Err(err) => error!("Failed to send response {}", err),
                }
            })
            .await;
        //??
        Ok(())
    }

    async fn dispatch_frame(
        &self,
        frame: TFrameId,
    ) -> Result<Option<(u32, FrameInput)>, ConnectionError> {
        let id = *frame.id();
        self.frame_dispatchers
            .dispatch(frame)
            .await
            .map(|receiver_option| receiver_option.map(|receiver| (id, receiver)))
    }

    async fn handle_init_handshake(
        &self,
        framed_read: &mut TFramedRead,
        framed_write: &mut TFramedWrite,
    ) -> Result<(), ConnectionError> {
        match framed_read.next().await {
            Some(Ok(mut frame_id)) => {
                Self::check_init_req(&mut frame_id).await?;
                Self::send_init_res(framed_write, *frame_id.id()).await?;
                Ok(())
            }
            Some(Err(err)) => Err(ConnectionError::FrameError(err)),
            None => Err(ConnectionError::Error(
                "Received no Init request.".to_string(),
            )),
        }
    }

    async fn check_init_req(frame_id: &mut TFrameId) -> Result<(), ConnectionError> {
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
        let init_frame_id =
            TFrameId::new(id, TFrame::new(Type::InitResponse, init.encode_bytes()?));
        Ok(framed_write.send(init_frame_id).await?)
    }

    async fn handle_request(
        &self,
        id: u32,
        frame_input: FrameInput,
    ) -> Result<Vec<TFrameId>, TChannelError> {
        let (request_fields, message_args) =
            RequestDefragmenter::new(frame_input).read_request().await?;
        //TODO start handling TTL here?
        self.frame_dispatchers.deregister(&id).await;
        let subchannel = self.get_subchannel(request_fields.service()).await?;
        let (response_code, response_args) = subchannel.handle(message_args).await?;
        Ok(ResponseFragmenter::new(
            request_fields.service().clone(),
            response_args,
            response_code,
        )
        .create_frames()?
        .map(|f| TFrameId::new(id, f))
        .collect::<Vec<TFrameId>>()
        .await)
    }

    async fn send_frames(
        &self,
        frames: Vec<TFrameId>,
        mut framed_write: FramedWrite<OwnedWriteHalf, TFrameIdCodec>,
    ) {
        for frame in frames {
            debug!("Writing frame (id: {})", frame.id());
            if let Err(err) = framed_write.send(frame).await {
                error!("Failed to write frame: {}", err);
            }
        }
        debug!("Flushing frames");
        if let Err(err) = framed_write.flush().await {
            error!("Failed to flush frames: {}", err);
        }
    }

    fn skip_if_err<T: Debug, Err: Debug>(
        result: Result<T, Err>,
    ) -> impl Future<Output = Option<T>> {
        match result {
            Ok(value) => {
                trace!("Got: {:?}", value);
                future::ready(Some(value))
            }
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

    async fn get_subchannel<STR: AsRef<str>>(
        &self,
        service: STR,
    ) -> Result<Arc<SubChannel>, TChannelError> {
        let subchannels = self.subchannels.read().await;
        subchannels.get(service.as_ref()).cloned().ok_or_else(|| {
            TChannelError::Error(format!("Failed to find subchannel '{}'", service.as_ref()))
        })
    }
}
