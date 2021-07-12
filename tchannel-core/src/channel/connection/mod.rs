use crate::channel::frames::payloads::ErrorMsg;
use crate::channel::frames::payloads::Init;
use crate::channel::frames::payloads::{Codec, PROTOCOL_VERSION};
use crate::channel::frames::{TFrame, TFrameId, TFrameIdCodec, Type};
use crate::TChannelError;
use async_trait::async_trait;
use bb8::{ErrorSink, ManageConnection, Pool, PooledConnection, RunError};
use bytes::BytesMut;
use core::time::Duration;
use futures::prelude::*;
use futures::stream::{self, Chunks, Iter};
use futures::{self, SinkExt, TryStreamExt};
use futures::{future, StreamExt};
use log::{debug, error, info, warn};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::error::Error;
use std::fmt::Debug;
use std::future::Future;
use std::net::SocketAddr;
use std::ops::Deref;
use std::pin::Pin;
use std::process::Output;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf, ReadHalf, WriteHalf};
use tokio::net::TcpStream;
use tokio::net::ToSocketAddrs;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::RwLock;
use tokio::sync::{oneshot, Mutex};
use tokio::task::JoinHandle;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::codec::{FramedRead, FramedWrite};

#[derive(Debug, Builder, Clone)]
pub struct ConnectionOptions {
    max_connections: u32,
    lifetime: Option<Duration>,
    test_connection: bool,
    frame_buffer_size: usize,
}

impl Default for ConnectionOptions {
    fn default() -> Self {
        ConnectionOptions {
            max_connections: 1,
            lifetime: Some(Duration::from_secs(60)),
            test_connection: false,
            frame_buffer_size: 100,
        }
    }
}

#[derive(Default, Debug)]
pub struct PendingIds {
    channels: RwLock<HashMap<u32, Sender<TFrameId>>>,
}

impl PendingIds {
    pub async fn add(&self, id: u32, sender: Sender<TFrameId>) {
        let mut channels = self.channels.write().await;
        channels.insert(id, sender);
    }

    pub async fn respond(&self, response: TFrameId) -> Result<(), TChannelError> {
        let id = response.id().clone();
        let mut channels = self.channels.read().await;
        if let Some(sender) = channels.get(&id) {
            return Ok(sender.send(response).await?);
        }
        Err(TChannelError::from(format!("Id {} not found", id)))
    }

    pub async fn remove(&self, id: &u32) -> Option<Sender<TFrameId>> {
        let mut channels = self.channels.write().await;
        channels.remove(id)
    }
}

#[derive(Debug, Default, Builder)]
#[builder(pattern = "owned")]
pub struct ConnectionPools {
    connection_options: ConnectionOptions,
    #[builder(setter(skip))]
    pools: RwLock<HashMap<SocketAddr, Arc<Pool<ConnectionManager>>>>,
    #[builder(setter(skip))]
    connection_pools_logger: ConnectionPoolsLogger,
}

impl ConnectionPools {
    //TODO do not like name
    pub async fn get(
        &self,
        addr: SocketAddr,
    ) -> Result<Arc<Pool<ConnectionManager>>, RunError<TChannelError>> {
        if let Some(pool) = self.pools.read().await.get(&addr) {
            return Ok(pool.clone());
        }
        self.create_pool(addr).await
    }

    async fn create_pool(
        &self,
        addr: SocketAddr,
    ) -> Result<Arc<Pool<ConnectionManager>>, RunError<TChannelError>> {
        let mut pools = self.pools.write().await;
        if let Some(pool) = pools.get(&addr) {
            return Ok(pool.clone());
        }
        let pool = Arc::new(
            Pool::builder()
                .max_lifetime(self.connection_options.lifetime)
                .max_size(self.connection_options.max_connections)
                .test_on_check_out(self.connection_options.test_connection)
                .error_sink(self.connection_pools_logger.boxed_clone())
                .build(ConnectionManager {
                    addr,
                    frame_buffer_size: self.connection_options.frame_buffer_size,
                })
                .await?,
        );
        pools.insert(addr, pool.clone());
        return Ok(pool);
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct ConnectionPoolsLogger;

impl<E> ErrorSink<E> for ConnectionPoolsLogger
where
    E: Debug,
{
    fn sink(&self, error: E) {
        error!("Connection error {:?}", error) //TODO fix Display impl
    }

    fn boxed_clone(&self) -> Box<dyn ErrorSink<E>> {
        Box::new(*self)
    }
}

#[derive(Debug, Builder)]
#[builder(pattern = "owned")]
pub struct Connection {
    #[builder(setter(skip))]
    next_message_id: AtomicU32,
    #[builder(setter(skip))]
    pending_ids: Arc<PendingIds>,
    sender: tokio::sync::mpsc::Sender<TFrameId>,
}

impl Connection {
    pub async fn connect(
        addr: SocketAddr,
        buffer_size: usize,
    ) -> Result<Connection, TChannelError> {
        debug!("Connecting to {}", addr);
        let mut tcpStream = TcpStream::connect(addr).await?;
        let (read, write) = tcpStream.into_split();
        let mut framed_read = FramedRead::new(read, TFrameIdCodec {});
        let mut framed_write = FramedWrite::new(write, TFrameIdCodec {});
        let (sender, receiver) = mpsc::channel::<TFrameId>(buffer_size);
        let mut connection = ConnectionBuilder::default().sender(sender).build()?;
        FrameReceiver::spawn(framed_read, connection.pending_ids.clone());
        FrameSender::spawn(framed_write, receiver, buffer_size);
        Ok(connection)
    }

    pub async fn send_one(&self, frame: TFrame) -> Result<TFrameId, TChannelError> {
        let (frame_output, mut frame_receiver) = self.send().await;
        frame_output.send(frame).await?;
        let response = frame_receiver.recv().await;
        frame_output.close().await;
        response.ok_or_else(|| TChannelError::Error("Received no response".to_owned()))
    }

    //TODO better name? pass output as an fn arg?
    pub async fn send_many(&self) -> (FrameOutput, FrameInput) {
        let (frame_output, mut frame_receiver) = self.send().await;
        let receiver_stream = ReceiverStream::new(frame_receiver);
        (frame_output, receiver_stream)
    }

    async fn send(&self) -> (FrameOutput, Receiver<TFrameId>) {
        let message_id = self.next_message_id();
        let (sender, mut receiver) = mpsc::channel::<TFrameId>(10); //TODO connfigure
        self.pending_ids.add(message_id.clone(), sender).await;
        let frame_output =
            FrameOutput::new(message_id, self.sender.clone(), self.pending_ids.clone());
        (frame_output, receiver)
    }

    fn next_message_id(&self) -> u32 {
        self.next_message_id.fetch_add(1, Ordering::Relaxed)
    }
}

pub type FrameInput = ReceiverStream<TFrameId>;

#[derive(Getters, new)]
pub struct FrameOutput {
    message_id: u32,
    sender: Sender<TFrameId>,
    pending_ids: Arc<PendingIds>,
}

impl FrameOutput {
    pub async fn send(&self, frame: TFrame) -> Result<(), TChannelError> {
        let frame = TFrameId::new(self.message_id, frame);
        debug!("Passing frame {:?} to sender.", frame);
        Ok(self.sender.send(frame).await?)
    }

    //TODO figure out how to automatically close it? impl Sink? do it on Deref?
    pub async fn close(&self) {
        self.pending_ids.remove(&self.message_id).await;
    }
}

struct FrameSender {
    framed_write: Arc<Mutex<FramedWrite<OwnedWriteHalf, TFrameIdCodec>>>,
    buffer_size: usize,
}

impl FrameSender {
    pub fn spawn(
        framed_write: FramedWrite<OwnedWriteHalf, TFrameIdCodec>,
        frame_receiver: tokio::sync::mpsc::Receiver<TFrameId>,
        buffer_size: usize,
    ) -> JoinHandle<()> {
        let mut frame_sender = FrameSender {
            framed_write: Arc::new(Mutex::new(framed_write)),
            buffer_size,
        };
        tokio::spawn(async move {
            frame_sender
                .run(ReceiverStream::new(frame_receiver))
                .then(|_| async { debug!("FrameSender stopped") })
                .await
        })
    }

    async fn run(&self, receiver_stream: ReceiverStream<TFrameId>) {
        debug!("Starting FrameSender");
        receiver_stream
            .ready_chunks(self.buffer_size)
            .for_each(|frames| self.send_frames(frames))
            .await
    }

    async fn send_frames(&self, frames: Vec<TFrameId>) {
        let mut framed_write = self.framed_write.lock().await;
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
}

struct FrameReceiver {
    pending_ids: Arc<PendingIds>,
}

impl FrameReceiver {
    pub fn spawn(
        framed_read: FramedRead<OwnedReadHalf, TFrameIdCodec>,
        pending_ids: Arc<PendingIds>,
    ) -> JoinHandle<()> {
        let mut frame_receiver = FrameReceiver { pending_ids };
        tokio::spawn(async move {
            frame_receiver
                .run(framed_read)
                .then(|_| async { debug!("FrameReceiver stopped") })
                .await
        })
    }

    async fn run(&self, frame_input: FramedRead<OwnedReadHalf, TFrameIdCodec>) {
        debug!("Starting FrameReceiver");
        frame_input
            .filter_map(|frame_res| match frame_res {
                Ok(frame) => future::ready(Some(frame)),
                Err(err) => {
                    error!("Failed to serialize frame: {:?}", err);
                    future::ready(None)
                }
            })
            .map(|frame| {
                debug!("Received frame id: {}", frame.id());
                self.pending_ids.respond(frame)
            })
            .for_each(|res| async {
                if let Some(err) = res.await.err() {
                    error!("Failed to send frame: {:?}", err);
                }
            })
            .await
    }
}

pub struct ConnectionManager {
    addr: SocketAddr,
    frame_buffer_size: usize,
}

#[async_trait]
impl bb8::ManageConnection for ConnectionManager {
    type Connection = Connection;
    type Error = TChannelError;

    async fn connect(&self) -> Result<Self::Connection, Self::Error> {
        let connection = Connection::connect(self.addr, self.frame_buffer_size).await?;
        Self::verify(connection).await
    }

    async fn is_valid(&self, conn: &mut PooledConnection<'_, Self>) -> Result<(), Self::Error> {
        error!("Is valid?");
        Ok(())
    }

    fn has_broken(&self, conn: &mut Self::Connection) -> bool {
        error!("Has broken?");
        false
    }
}

impl ConnectionManager {
    async fn verify(connection: Connection) -> Result<Connection, TChannelError> {
        let mut frame_id = Self::init_handshake(&connection).await?;
        match frame_id.frame().frame_type() {
            Type::InitResponse => {
                let init = Init::decode(frame_id.frame_mut().payload_mut())?;
                debug!("Received Init response: {:?}", init);
                match *init.version() {
                    PROTOCOL_VERSION => Ok(connection),
                    other_version => Err(TChannelError::Error(format!(
                        "Unsupported protocol version: {} ",
                        other_version
                    ))),
                }
            }
            Type::Error => {
                let error = ErrorMsg::decode(frame_id.frame_mut().payload_mut())?;
                debug!("Received error response {:?}", error);
                return Err(TChannelError::ResponseError(error));
            }
            other_type => return Err(TChannelError::UnexpectedResponseError(*other_type)),
        }
    }

    async fn init_handshake(connection: &Connection) -> Result<TFrameId, TChannelError> {
        let mut bytes = BytesMut::new();
        let init = Init::default();
        init.encode(&mut bytes);
        let frame = TFrame::new(Type::InitRequest, bytes.freeze());
        connection.send_one(frame).await
    }
}
