use crate::channel::frames::{TFrame, TFrameCodec};
use crate::TChannelError;
use async_trait::async_trait;
use bb8::{ManageConnection, Pool, PooledConnection, RunError};
use bytes::BytesMut;
use core::time::Duration;
use std::collections::HashMap;
use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::net::ToSocketAddrs;
use tokio::sync::RwLock;
use tokio_util::codec::Framed;

#[derive(Debug, Builder, Clone)]
pub struct ConnectionOptions {
    max_connections: u32,
    lifetime: Option<Duration>,
}

impl Default for ConnectionOptions {
    fn default() -> Self {
        ConnectionOptions {
            max_connections: 1,
            lifetime: Some(Duration::from_secs(600)),
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
        let pools = self.pools.read().await;
        match pools.get(&addr) {
            Some(pool) => Ok(pool.clone()),
            None => self.add_pool(addr).await,
        }
    }

    async fn add_pool(
        &self,
        addr: SocketAddr,
    ) -> Result<Arc<Pool<ConnectionManager>>, RunError<TChannelError>> {
        let mut pools = self.pools.write().await;
        match pools.get(&addr) {
            Some(pool) => Ok(pool.clone()),
            None => {
                let client = ConnectionManager { addr };
                let pool = Arc::new(
                    Pool::builder()
                        .max_lifetime(self.connection_options.lifetime)
                        .max_size(self.connection_options.max_connections)
                        .build(client)
                        .await?,
                );
                pools.insert(addr, pool.clone());
                return Ok(pool);
            }
        }
    }
}

type TFrameStream = Framed<TcpStream, TFrameCodec>;

struct Connection {
    stream: TFrameStream,
}

pub struct ConnectionManager {
    addr: SocketAddr,
}

#[async_trait]
impl bb8::ManageConnection for ConnectionManager {
    type Connection = ();
    type Error = TChannelError;

    async fn connect(&self) -> Result<Self::Connection, Self::Error> {
        todo!()
    }

    async fn is_valid(&self, conn: &mut PooledConnection<'_, Self>) -> Result<(), Self::Error> {
        todo!()
    }

    fn has_broken(&self, conn: &mut Self::Connection) -> bool {
        todo!()
    }
}

#[derive(Debug)]
pub struct Peer {
    // pub message_stream: Framed<FrameStream, MessageCodec>,
    frame_codec: Framed<TcpStream, TFrameCodec>,
}

impl From<TcpStream> for Peer {
    fn from(tcp_stream: TcpStream) -> Self {
        // let tframe_stream = Framed::new(tcp_stream, TFrameCodec);
        // let frame_stream = Framed::new(tframe_stream, FrameCodec);
        Peer {
            // message_stream: Framed::new(frame_stream, MessageCodec),
            frame_codec: Framed::new(tcp_stream, TFrameCodec {}),
        }
    }
}
