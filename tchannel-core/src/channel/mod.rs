pub mod connection;
pub mod frames;
pub mod messages;

use crate::channel::connection::{ConnectionOptions, ConnectionPools, ConnectionPoolsBuilder};
use crate::channel::frames::{TFrame, TFrameBuilder, Type};
use crate::channel::messages::{Message, MessageCodec, Request, Response};
use crate::handlers::RequestHandler;
use crate::{Error, TChannelError};
use bytes::{Bytes, BytesMut};
use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::net::ToSocketAddrs;
use tokio::sync::RwLock;
use tokio_util::codec::{Decoder, Encoder, Framed};

pub struct TChannel {
    subchannels: RwLock<HashMap<String, Arc<SubChannel>>>,
    connection_pools: Arc<ConnectionPools>,
}

impl TChannel {
    pub fn new(connection_options: ConnectionOptions) -> Result<Self, TChannelError> {
        let connection_pools = ConnectionPoolsBuilder::default()
            .connection_options(connection_options)
            .build()?;
        Ok(TChannel {
            subchannels: RwLock::new(HashMap::new()),
            connection_pools: Arc::new(connection_pools),
        })
    }

    pub async fn subchannel(&mut self, service_name: String) -> Result<Arc<SubChannel>, String> {
        if let Some(subchannel) = self.subchannels.read().await.get(&service_name) {
            return Ok(subchannel.clone());
        }
        self.make_subchannel(service_name).await
    }

    async fn make_subchannel(&self, service_name: String) -> Result<Arc<SubChannel>, String> {
        let mut subchannels = self.subchannels.write().await;
        match subchannels.get(&service_name) {
            Some(subchannel) => Ok(subchannel.clone()),
            None => {
                let subchannel = Arc::new(
                    SubChannelBuilder::default()
                        .service_name(service_name.to_string())
                        .connection_pools(self.connection_pools.clone())
                        .build()?,
                );
                subchannels.insert(service_name, subchannel.clone());
                Ok(subchannel)
            }
        }
    }
}

#[derive(Debug, Default, Builder)]
#[builder(pattern = "owned")]
pub struct SubChannel {
    service_name: String,
    connection_pools: Arc<ConnectionPools>,
    #[builder(setter(skip))]
    next_message_id: AtomicU32,
    #[builder(setter(skip))]
    handlers: HashMap<String, Box<RequestHandler>>,
}

impl SubChannel {
    pub fn register<HANDLER>(&mut self, handler_name: &str, handler: HANDLER) -> &Self {
        //TODO
        self
    }

    async fn send<REQ: Request + TryInto<TFrame>, RES: Response + TryFrom<TFrame>>(
        &self,
        request: REQ,
        host: SocketAddr,
        port: u16,
    ) -> Result<RES, crate::TChannelError> {
        let pool = self.connection_pools.get_or_add(host).await?;
        let connection = pool.get().await?;
        let messsage_id = self.next_message_id();
        // write to message stream
        println!("unimplemented");
        Err(TChannelError::from("unimplemented".to_string()))
    }

    fn next_message_id(&self) -> u32 {
        self.next_message_id.fetch_add(1, Ordering::Relaxed)
    }
}
