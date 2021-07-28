use crate::connection::{Connection, ConnectionOptions};
use crate::errors::ConnectionError;
use crate::errors::ConnectionError::{MessageError, UnexpectedResponseError};
use crate::frames::payloads::ErrorMsg;
use crate::frames::payloads::Init;
use crate::frames::payloads::{Codec, PROTOCOL_VERSION};
use crate::frames::{TFrame, TFrameId, TFrameIdCodec, Type};
use async_trait::async_trait;
use bb8::{ErrorSink, Pool, PooledConnection, RunError};
use bytes::{Bytes, BytesMut};
use core::time::Duration;
use futures::prelude::*;
use futures::{self, SinkExt};
use futures::{future, StreamExt};
use log::{debug, error};
use std::collections::HashMap;
use std::fmt::Debug;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::Mutex;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::codec::{FramedRead, FramedWrite};

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
    type Connection = Connection;
    type Error = ConnectionError;

    async fn connect(&self) -> Result<Self::Connection, Self::Error> {
        let connection = Connection::connect(self.addr, self.frame_buffer_size).await?;
        verify(connection).await
    }

    async fn is_valid(&self, conn: &mut PooledConnection<'_, Self>) -> Result<(), Self::Error> {
        debug!("Is valid?");
        ping(conn).await
    }

    fn has_broken(&self, conn: &mut Self::Connection) -> bool {
        debug!("Has broken? (not implemented)");
        false
    }
}

async fn verify(connection: Connection) -> Result<Connection, ConnectionError> {
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
            Err(ConnectionError::MessageError(error))
        }
        other_type => Err(ConnectionError::UnexpectedResponseError(*other_type)),
    }
}

async fn init_handshake(connection: &Connection) -> Result<TFrameId, ConnectionError> {
    let mut bytes = BytesMut::new();
    let init = Init::default();
    init.encode(&mut bytes);
    let init_frame = TFrame::new(Type::InitRequest, bytes.freeze());
    connection.send_one(init_frame).await
}

async fn ping(connection: &Connection) -> Result<(), ConnectionError> {
    let ping_req = TFrame::new(Type::PingRequest, Bytes::new());
    connection
        .send_one(ping_req)
        .await
        .map(|frame_id| frame_id.frame)
        .map(|mut frame| match frame.frame_type() {
            Type::PingResponse => Ok(()),
            Type::Error => Err(MessageError(ErrorMsg::decode(frame.payload_mut())?)),
            frame_type => Err(UnexpectedResponseError(frame_type.clone())),
        })?
}
