use crate::config::Config;
use crate::connection::{Connection, ConnectionResult};
use crate::errors::ConnectionError;
use crate::errors::ConnectionError::{MessageErrorId, UnexpectedResponseError};
use crate::frames::payloads::ErrorMsg;
use crate::frames::payloads::Init;
use crate::frames::payloads::{Codec, PROTOCOL_VERSION};
use crate::frames::{TFrame, TFrameId, Type};
use async_trait::async_trait;
use bb8::{ErrorSink, Pool, RunError};
use bytes::Bytes;
use log::{debug, error};
use std::collections::HashMap;
use std::fmt::Debug;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::RwLock;

#[derive(Debug, Default, new)]
pub struct ConnectionPools {
    config: Arc<Config>,
    #[new(default)]
    pools: RwLock<HashMap<SocketAddr, Arc<Pool<ConnectionManager>>>>,
    #[new(default)]
    connection_pools_logger: ConnectionPoolsLogger,
}

impl ConnectionPools {
    pub async fn get(
        &self,
        addr: SocketAddr,
    ) -> Result<Arc<Pool<ConnectionManager>>, RunError<ConnectionError>> {
        if let Some(pool) = self.pools.read().await.get(&addr) {
            return Ok(pool.clone());
        }
        self.create_pool(addr).await
    }

    async fn create_pool(
        &self,
        addr: SocketAddr,
    ) -> Result<Arc<Pool<ConnectionManager>>, RunError<ConnectionError>> {
        debug!("Creating connection pool for '{}'", addr);
        let mut pools = self.pools.write().await;
        if let Some(pool) = pools.get(&addr) {
            return Ok(pool.clone());
        }
        let pool = Arc::new(
            Pool::builder()
                .max_lifetime(self.config.lifetime)
                .max_size(self.config.max_connections)
                .test_on_check_out(self.config.test_connection)
                .error_sink(self.connection_pools_logger.boxed_clone())
                .build(ConnectionManager {
                    addr,
                    frame_buffer_size: self.config.frame_buffer_size,
                })
                .await?,
        );
        pools.insert(addr, pool.clone());
        Ok(pool)
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

pub struct ConnectionManager {
    addr: SocketAddr,
    frame_buffer_size: usize,
}

#[async_trait]
impl bb8::ManageConnection for ConnectionManager {
    //TODO create in out connection types?
    type Connection = Connection;
    type Error = ConnectionError;

    async fn connect(&self) -> Result<Self::Connection, Self::Error> {
        debug!("Connecting to {}", self.addr);
        let stream = TcpStream::connect(self.addr).await?;
        let connection = Connection::connect(stream, self.frame_buffer_size).await?;
        verify(connection).await
    }

    async fn is_valid(&self, conn: &mut Self::Connection) -> Result<(), Self::Error> {
        debug!("Is valid?");
        ping(conn).await
    }

    fn has_broken(&self, _conn: &mut Self::Connection) -> bool {
        debug!("Has broken? (not implemented)");
        false
    }
}

async fn verify(connection: Connection) -> ConnectionResult<Connection> {
    let mut frame_id = init_handshake(&connection).await?;
    match frame_id.frame().frame_type() {
        Type::InitResponse => {
            let init = Init::decode(frame_id.frame.payload_mut())?;
            debug!("Received Init response: {:?}", init);
            match *init.version() {
                PROTOCOL_VERSION => Ok(connection),
                other_version => Err(ConnectionError::Error(format!(
                    "Unsupported protocol version: {} ",
                    other_version
                ))),
            }
        }
        Type::Error => {
            let error = ErrorMsg::decode(frame_id.frame.payload_mut())?;
            debug!("Received error response {:?}", error);
            Err(ConnectionError::MessageErrorId(error, *frame_id.id()))
        }
        other_type => Err(ConnectionError::UnexpectedResponseError(*other_type)),
    }
}

async fn init_handshake(connection: &Connection) -> ConnectionResult<TFrameId> {
    debug!("Init handshake.");
    let init = Init::default();
    //TODO add proper `crate::frames::headers::InitHeaderKey` headers
    let init_frame = TFrame::new(Type::InitRequest, init.encode_bytes()?);
    connection.send_one(init_frame).await
}

async fn ping(connection: &Connection) -> ConnectionResult<()> {
    let ping_req = TFrame::new(Type::PingRequest, Bytes::new());
    connection
        .send_one(ping_req)
        .await
        .map(|mut frame_id| match frame_id.frame.frame_type() {
            Type::PingResponse => Ok(()),
            Type::Error => Err(MessageErrorId(
                ErrorMsg::decode(frame_id.frame.payload_mut())?,
                *frame_id.id(),
            )),
            frame_type => Err(UnexpectedResponseError(*frame_type)),
        })?
}
