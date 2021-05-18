use crate::channel::frames::payloads::Codec;
use crate::channel::frames::payloads::Init;
use crate::channel::frames::payloads::InitBuilder;
use crate::channel::frames::{TFrame, TFrameBuilder, TFrameCodec, Type};
use crate::TChannelError;
use async_trait::async_trait;
use bb8::{ManageConnection, Pool, PooledConnection, RunError};
use bytes::BytesMut;
use core::time::Duration;
use futures::stream::StreamExt;
use futures::SinkExt;
use std::collections::HashMap;
use std::future::Future;
use std::net::SocketAddr;
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
    #[builder(setter(skip))]
    pending_ids: PendingIds,
}

impl ConnectionsHandler {
    pub async fn get_or_add(
        &self,
        addr: SocketAddr,
    ) -> Result<Arc<Pool<ConnectionManager>>, RunError<TChannelError>> {
        // TODO dummy impl
        self.connection_pools.get_or_add(addr).await
    }

    pub async fn send(&mut self, frame: TFrame, addr: SocketAddr) -> Result<TFrame, TChannelError> {
        let pool = self.connection_pools.get_or_add(addr).await?;
        let mut connection = pool.get().await?;
        connection.frame_output.send(frame).await;
        unimplemented!()
    }
}

#[derive(Default, Debug)]
pub struct PendingIds {
    channels: HashMap<u32, Sender<TFrame>>,
}

impl PendingIds {
    pub fn add(&mut self, id: u32, channel: Sender<TFrame>) {
        self.channels.insert(id, channel);
    }

    pub fn respond(&mut self, response: TFrame) -> Result<(), TFrame> {
        if let Some(sender) = self.channels.remove(&response.id()) {
            sender.send(response)
        } else {
            Err(response) //?
        }
    }
}

#[derive(Debug, Default, Builder)]
#[builder(pattern = "owned")]
pub struct ConnectionPools {
    connection_options: ConnectionOptions,
    #[builder(setter(skip))]
    pools: RwLock<HashMap<SocketAddr, Arc<Pool<ConnectionManager>>>>,
}

// 1. create subchannel dedicated wrapper around connection pools
// 2. store pending id-res_channel map there
// 3. store there subchannel dedicated connection manager using id-resp_channel map to handle init handshake

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

pub struct Connection {
    frame_input: FramedRead<OwnedReadHalf, TFrameCodec>,
    frame_output: FramedWrite<OwnedWriteHalf, TFrameCodec>,
    sender: tokio::sync::mpsc::Sender<TFrame>,
}

impl Connection {
    pub async fn new(addr: SocketAddr, buffer_size: usize) -> Result<Connection, TChannelError> {
        let mut tcpStream = TcpStream::connect(addr).await?;
        let (read, write) = tcpStream.into_split();
        let mut frame_input = FramedRead::new(read, TFrameCodec {});
        let mut frame_output = FramedWrite::new(write, TFrameCodec {});
        let (sender, mut receiver) = mpsc::channel::<TFrame>(buffer_size);
        let connection = Connection {
            frame_input,
            frame_output,
            sender,
        };
        let receiver_stream = ReceiverStream::new(receiver);
        unimplemented!()
    }

    pub async fn send(&mut self, frame: TFrame) -> Result<(), TChannelError> {
        self.frame_output.send(frame).await
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
