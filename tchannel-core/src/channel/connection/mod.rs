use crate::channel::frames::payloads::Codec;
use crate::channel::frames::payloads::Init;
use crate::channel::frames::payloads::InitBuilder;
use crate::channel::frames::{TFrame, TFrameBuilder, TFrameCodec, Type};
use crate::TChannelError;
use async_trait::async_trait;
use bb8::{ManageConnection, Pool, PooledConnection, RunError};
use bytes::BytesMut;
use core::time::Duration;
use futures::prelude::*;
use futures::stream::{self, Chunks, Iter};
use futures::{self, SinkExt, TryStreamExt};
use futures::{future, StreamExt};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
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
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::codec::{FramedRead, FramedWrite};

#[derive(Debug, Builder, Clone)]
pub struct ConnectionOptions {
    max_connections: u32,
    lifetime: Option<Duration>,
    test_connection: bool,
}

impl Default for ConnectionOptions {
    fn default() -> Self {
        ConnectionOptions {
            max_connections: 1,
            lifetime: Some(Duration::from_secs(600)),
            test_connection: false,
        }
    }
}

#[derive(Debug, Builder)]
#[builder(pattern = "owned")]
pub struct ConnectionsHandler {
    connection_pools: Arc<ConnectionPools>,
}

impl ConnectionsHandler {
    pub async fn get_or_add(
        &self,
        addr: SocketAddr,
    ) -> Result<Arc<Pool<ConnectionManager>>, RunError<TChannelError>> {
        // TODO dummy impl
        self.connection_pools.get_or_add(addr).await
    }

    pub async fn send(&self, frame: TFrame, addr: SocketAddr) -> Result<TFrame, TChannelError> {
        let pool = self.connection_pools.get_or_add(addr).await?;
        let mut connection = pool.get().await?;
        connection.send(frame).await
    }
}

#[derive(Default, Debug)]
pub struct PendingIds {
    channels: Mutex<HashMap<u32, Sender<TFrame>>>,
}

impl PendingIds {
    pub async fn add(&self, id: u32, channel: Sender<TFrame>) {
        let mut channels = self.channels.lock().await;
        channels.insert(id, channel);
    }

    pub async fn respond(&self, response: TFrame) -> Result<(), TChannelError> {
        let id = response.id().clone();
        if let Some(sender) = self.remove(&id).await {
            sender.send(response)?
        }
        //TODO use proper type of error
        Err(TChannelError::from(format!("Id {} not found", id)))
    }

    async fn remove(&self, id: &u32) -> Option<Sender<TFrame>> {
        let mut channels = self.channels.lock().await;
        channels.remove(id)
    }
}

#[derive(Debug, Default, Builder)]
#[builder(pattern = "owned")]
pub struct ConnectionPools {
    connection_options: ConnectionOptions,
    #[builder(setter(skip))]
    pools: RwLock<HashMap<SocketAddr, Arc<Pool<ConnectionManager>>>>,
}

impl ConnectionPools {
    pub async fn get_or_add(
        &self,
        addr: SocketAddr,
    ) -> Result<Arc<Pool<ConnectionManager>>, RunError<TChannelError>> {
        if let Some(pool) = self.pools.read().await.get(&addr) {
            return Ok(pool.clone());
        }
        self.add_pool(addr).await
    }

    async fn add_pool(
        &self,
        addr: SocketAddr,
    ) -> Result<Arc<Pool<ConnectionManager>>, RunError<TChannelError>> {
        let mut pools = self.pools.write().await;
        if let Some(pool) = pools.get(&addr) {
            return Ok(pool.clone());
        }
        let connection_manager = ConnectionManager { addr };
        let pool = Arc::new(
            Pool::builder()
                .max_lifetime(self.connection_options.lifetime)
                .max_size(self.connection_options.max_connections)
                .test_on_check_out(self.connection_options.test_connection)
                .build(connection_manager)
                .await?,
        );
        pools.insert(addr, pool.clone());
        return Ok(pool);
    }
}

#[derive(Debug, Builder)]
#[builder(pattern = "owned")]
pub struct Connection {
    #[builder(setter(skip))]
    next_message_id: AtomicU32,
    #[builder(setter(skip))]
    pending_ids: Arc<PendingIds>,
    output_sender: tokio::sync::mpsc::Sender<TFrame>,
}

impl Connection {
    pub fn next_message_id(&self) -> u32 {
        self.next_message_id.fetch_add(1, Ordering::Relaxed)
    }

    pub async fn connect(
        addr: SocketAddr,
        buffer_size: usize,
    ) -> Result<Connection, TChannelError> {
        let mut tcpStream = TcpStream::connect(addr).await?;
        let (read, write) = tcpStream.into_split();
        let mut framed_read = FramedRead::new(read, TFrameCodec {});
        let mut framed_write = FramedWrite::new(write, TFrameCodec {});
        let (frame_sender, mut frame_receiver) = mpsc::channel::<TFrame>(buffer_size);
        FrameSender::spawn(framed_write, frame_receiver, buffer_size);
        //TODO keep reference to shutdown it?
        let mut connection = ConnectionBuilder::default()
            .output_sender(frame_sender)
            .build()?;
        connection.start_input_handling(framed_read);
        Ok(connection)
    }

    fn start_input_handling(&mut self, frame_input: FramedRead<OwnedReadHalf, TFrameCodec>) {
        let pending_ids = self.pending_ids.clone();
        tokio::spawn(async move { Self::handle_input(frame_input, pending_ids).await });
    }

    async fn handle_input(
        frame_input: FramedRead<OwnedReadHalf, TFrameCodec>,
        pending_ids: Arc<PendingIds>,
    ) {
        frame_input
            .filter_map(|frame_res| match frame_res {
                Ok(frame) => future::ready(Some(frame)),
                Err(err) => {
                    //TODO log
                    println!("Failed to serialize frame: {:?}", err);
                    future::ready(None)
                }
            })
            .map(|frame| pending_ids.respond(frame))
            .for_each(|res| async {
                if let Some(err) = res.await.err() {
                    //TODO log
                    println!("Failed to send frame {:?}", err);
                }
            });
    }

    pub async fn send(&mut self, frame: TFrame) -> Result<TFrame, TChannelError> {
        let (tx, mut rx) = oneshot::channel::<TFrame>();
        self.pending_ids.add(*frame.id(), tx);
        self.output_sender.send(frame).await;
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
    ) {
        tokio::spawn(async move {
            FrameSender {
                framed_write: Arc::new(Mutex::new(framed_write)),
                buffer_size,
            }
            .run(ReceiverStream::new(frame_receiver))
            .await
        });
    }

    pub async fn run(&mut self, receiver_stream: ReceiverStream<TFrame>) {
        receiver_stream
            .chunks(self.buffer_size)
            .for_each(|frames| self.send_frames(frames))
            .await;
    }

    async fn send_frames(&self, frames: Vec<TFrame>) {
        let mut framed_write = self.framed_write.lock().await;
        for frame in frames {
            if let Err(err) = framed_write.feed(frame).await {
                println!("Frame writing error {}", err);
                //TODO logging
            }
        }
        if let Err(err) = framed_write.flush().await {
            println!("Frame flushing error {}", err);
            //TODO logging
        }
    }
}

pub struct ConnectionManager {
    addr: SocketAddr,
}

#[async_trait]
impl bb8::ManageConnection for ConnectionManager {
    type Connection = Connection;
    type Error = TChannelError;

    async fn connect(&self) -> Result<Self::Connection, Self::Error> {
        let mut tcpStream = TcpStream::connect(self.addr).await?;
        let (read, write) = tcpStream.into_split();
        let mut frame_input = FramedRead::new(read, TFrameCodec {});
        let mut frame_output = FramedWrite::new(write, TFrameCodec {});
        /*
        let (tx, mut rx) = oneshot::channel::<Result<TFrame, TChannelError>>();
        match rx.await {
            Ok(response) => println!("Got response {}", response.id()),
            Err(err) => println!("Got error {}", err),
        }
        */
        // dummy implementation
        /*
        let mut bytes = BytesMut::new();
        let init = InitBuilder::default().build()?;
        init.encode(&mut bytes);
        let mut frame = TFrameBuilder::default()
            .id(1)
            .frame_type(Type::InitRequest)
            .payload(bytes.freeze())
            .build()?;
        println!("Sending");
        let sent = stream.send(frame).await;
        println!("Sent: {:?}", sent);
        while let Some(request) = stream.next().await {
            match request {
                Ok(frame) => match frame.frame_type() {
                    Type::InitResponse => {
                        let init = Init::decode(&mut frame.payload().clone())?;
                        println!("Incoming init frame: {:?}", init);
                    }
                    frame_type => println!("Incoming frame: {:?}", frame_type),
                },
                Err(e) => return Err(e.into()),
            }
        }
        */
        //
        // Ok(Connection {
        //     frame_input,
        //     frame_output,
        // })
        unimplemented!()
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
