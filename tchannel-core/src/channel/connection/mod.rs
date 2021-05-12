use crate::channel::frames::payloads::Codec;
use crate::channel::frames::payloads::Init;
use crate::channel::frames::payloads::InitBuilder;
use crate::channel::frames::{TFrame, TFrameBuilder, TFrameCodec, Type};
use crate::TChannelError;
use async_trait::async_trait;
use bb8::{ManageConnection, Pool, PooledConnection, RunError};
use bytes::BytesMut;
use core::time::Duration;
use futures::SinkExt;
use std::collections::HashMap;
use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::net::ToSocketAddrs;
use tokio::sync::RwLock;
use tokio_stream::StreamExt;
use tokio_util::codec::Framed;

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

#[derive(Debug, Default, Builder)]
#[builder(pattern = "owned")]
#[builder(build_fn(name = "build_internal"))]
pub struct ConnectionPools {
    connection_options: ConnectionOptions,
    #[builder(field(private))]
    pools: RwLock<HashMap<SocketAddr, Arc<Pool<ConnectionManager>>>>,
}

impl ConnectionPoolsBuilder {
    pub fn build(self) -> ::std::result::Result<ConnectionPools, String> {
        let pools = RwLock::new(HashMap::new());
        self.pools(pools).build_internal()
    }
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
        let client = ConnectionManager { addr };
        let pool = Arc::new(
            Pool::builder()
                .max_lifetime(self.connection_options.lifetime)
                .max_size(self.connection_options.max_connections)
                .test_on_check_out(self.connection_options.test_connection)
                .build(client)
                .await?,
        );
        pools.insert(addr, pool.clone());
        return Ok(pool);
    }
}

type TFrameStream = Framed<TcpStream, TFrameCodec>;

pub struct Connection {
    pub(self) stream: TFrameStream,
}

impl Connection {
    pub async fn send(&mut self, frame: TFrame) -> Result<(), TChannelError> {
        self.stream.send(frame).await
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
        let tcpStream = TcpStream::connect(self.addr).await?;
        let mut stream = Framed::new(tcpStream, TFrameCodec {});
        // dummy implementation
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
        //
        Ok(Connection { stream })
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
