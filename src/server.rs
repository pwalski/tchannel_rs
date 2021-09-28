use crate::channel::{SharedSubChannels, TResult};
use crate::connection::{Config, ConnectionResult, FrameInput, FrameSender, FramesDispatcher};
use crate::defragmentation::RequestDefragmenter;
use crate::errors::{ConnectionError, TChannelError};
use crate::fragmentation::ResponseFragmenter;
use crate::frames::headers::InitHeaderKey;
use crate::frames::payloads::Init;
use crate::frames::payloads::{Codec, ErrorMsg, PROTOCOL_VERSION};
use crate::frames::{TFrame, TFrameId, TFrameIdCodec, Type};
use crate::SubChannel;
use futures::SinkExt;
use futures::{self, Future};
use futures::{future, StreamExt};
use log::{debug, trace};
use std::array::IntoIter;
use std::collections::HashMap;
use std::fmt::Debug;
use std::iter::FromIterator;
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

    pub async fn run(config: Arc<Config>, subchannels: SharedSubChannels) -> ConnectionResult<()> {
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

    async fn handle_connection(&mut self, stream: TcpStream) -> ConnectionResult<()> {
        let (read, write) = stream.into_split();
        let mut framed_read = FramedRead::new(read, TFrameIdCodec {});
        let mut framed_write = FramedWrite::new(write, TFrameIdCodec {});

        self.handle_init_handshake(&mut framed_read, &mut framed_write)
            .await?;

        let (sender, receiver) = mpsc::channel::<TFrameId>(self.buffer_size);
        FrameSender::spawn(framed_write, receiver, self.buffer_size); //TODO should use same value?

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

    async fn dispatch_frame(&self, frame: TFrameId) -> ConnectionResult<Option<(u32, FrameInput)>> {
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
    ) -> ConnectionResult<()> {
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
        let headers = HashMap::from_iter(IntoIter::new([(
            InitHeaderKey::TChannelLanguage.to_string(),
            "rust".to_string(),
        )]));
        let init = Init::new(PROTOCOL_VERSION, headers);
        let init_frame_id =
            TFrameId::new(id, TFrame::new(Type::InitResponse, init.encode_bytes()?));
        Ok(framed_write.send(init_frame_id).await?)
    }

    async fn handle_request(&self, id: u32, frame_input: FrameInput) -> TResult<Vec<TFrameId>> {
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

    async fn get_subchannel<STR: AsRef<str>>(&self, service: STR) -> TResult<Arc<SubChannel>> {
        let subchannels = self.subchannels.read().await;
        subchannels.get(service.as_ref()).cloned().ok_or_else(|| {
            TChannelError::Error(format!("Failed to find subchannel '{}'", service.as_ref()))
        })
    }
}
