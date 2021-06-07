use crate::channel::frames::payloads::Codec;
use crate::channel::frames::payloads::Init;
use crate::channel::frames::payloads::InitBuilder;
use crate::channel::frames::{TFrame, TFrameBuilder, TFrameCodec, Type};
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
use std::process::Output;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf, ReadHalf, WriteHalf};
use tokio::net::TcpStream;
use tokio::net::ToSocketAddrs;
use tokio::sync::mpsc;
use tokio::sync::oneshot::{Receiver, Sender};
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
    channels: Mutex<HashMap<u32, Sender<TFrame>>>,
}

impl PendingIds {
    pub async fn add(&self, id: u32, sender: Sender<TFrame>) {
        let mut channels = self.channels.lock().await;
        channels.insert(id, sender);
    }

    pub async fn respond(&self, response: TFrame) -> Result<(), TChannelError> {
        let id = response.id().clone();
        if let Some(sender) = self.remove(&id).await {
            return Ok(sender.send(response)?);
        }
        Err(TChannelError::from(format!("Id {} not found", id)))
    }

    async fn remove(&self, id: &u32) -> Option<Sender<TFrame>> {
        let mut channels = self.channels.lock().await;
        debug!("Channels len {}", channels.len()); //TODO remove
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
    pub async fn get_or_create(
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
    sender: tokio::sync::mpsc::Sender<TFrame>,
}

impl Connection {
    pub fn next_message_id(&self) -> u32 {
        self.next_message_id.fetch_add(1, Ordering::Relaxed)
    }

    pub async fn connect(
        addr: SocketAddr,
        buffer_size: usize,
    ) -> Result<Connection, TChannelError> {
        debug!("Connecting to {}", addr);
        let mut tcpStream = TcpStream::connect(addr).await?;
        let (read, write) = tcpStream.into_split();
        let mut framed_read = FramedRead::new(read, TFrameCodec {});
        let mut framed_write = FramedWrite::new(write, TFrameCodec {});
        let (sender, receiver) = mpsc::channel::<TFrame>(buffer_size);
        let mut connection = ConnectionBuilder::default().sender(sender).build()?;
        FrameReceiver::spawn(framed_read, connection.pending_ids.clone());
        FrameSender::spawn(framed_write, receiver, buffer_size);
        Ok(connection)
    }

    pub async fn send(&self, frame: TFrame) -> Result<TFrame, TChannelError> {
        debug!("Sending frame (id {})", frame.id());
        let (tx, mut rx) = oneshot::channel::<TFrame>();
        self.pending_ids.add(frame.id().clone(), tx).await;
        self.sender.send(frame).await;
        debug!("Waiting for response");
        Ok(rx.await?)
    }
}

struct FrameSender {
    framed_write: Arc<Mutex<FramedWrite<OwnedWriteHalf, TFrameCodec>>>,
    buffer_size: usize,
}

impl FrameSender {
    pub fn spawn(
        framed_write: FramedWrite<OwnedWriteHalf, TFrameCodec>,
        frame_receiver: tokio::sync::mpsc::Receiver<TFrame>,
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

    async fn run(&self, receiver_stream: ReceiverStream<TFrame>) {
        debug!("Starting FrameSender");
        receiver_stream
            .ready_chunks(self.buffer_size)
            .for_each(|frames| self.send_frames(frames))
            .await
    }

    async fn send_frames(&self, frames: Vec<TFrame>) {
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
        framed_read: FramedRead<OwnedReadHalf, TFrameCodec>,
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

    async fn run(&self, frame_input: FramedRead<OwnedReadHalf, TFrameCodec>) {
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
        let connection = Connection::connect(self.addr, self.frame_buffer_size).await?; //TODO how to init/force ?

        let id = connection.next_message_id();
        let mut bytes = BytesMut::new();
        let init = InitBuilder::default().build()?;
        init.encode(&mut bytes);
        let mut frame = TFrameBuilder::default()
            .id(id)
            .frame_type(Type::InitRequest)
            .payload(bytes.freeze())
            .build()?;
        // let response = connection.send(frame).await?;
        match connection.send(frame).await {
            Ok(response) => Ok(connection),
            Err(err) => {
                println!("Err: {}", err);
                return Err(err);
            }
        }
        // debug!("Got response {:?}", response);
        // assert_eq!(*response.id(), id); //TODO handle it
        // debug!("OK");
        // Ok(connection)
    }

    async fn is_valid(&self, conn: &mut PooledConnection<'_, Self>) -> Result<(), Self::Error> {
        println!("Is valid?");
        todo!()
    }

    fn has_broken(&self, conn: &mut Self::Connection) -> bool {
        println!("Has broken?");
        todo!()
    }
}
