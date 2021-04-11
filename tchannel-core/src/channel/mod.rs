pub mod messages;

use crate::channel::messages::{Request, Response, ResponseBuilder};
use crate::connection::Connection;
use crate::handlers::RequestHandler;
use crate::Result;

use crate::frame::Type::Error;
use std::collections::HashMap;
use std::net::SocketAddr;
use tokio::net::TcpStream;
use tokio::net::ToSocketAddrs;

use std::ops::Deref;
// use tokio_util::codec::Framed;
use crate::channel::messages::raw::{RawRequest, RawResponse};
use crate::channel::messages::thrift::{ThriftRequest, ThriftResponse};
use crate::transport::TFrameCodec;
use std::borrow::BorrowMut;
use std::cell::{Cell, RefCell};
use std::sync::{Arc, Mutex, RwLock};

#[derive(Default, Builder)]
#[builder(pattern = "mutable")]
#[builder(build_fn(name = "build_internal"))]
pub struct TChannel {
    subchannels: HashMap<String, SubChannel>,
    connection_options: ConnectionOptions,
    #[builder(field(private))]
    pub(super) peers_pool: Arc<PeersPool>,
}

impl TChannelBuilder {
    pub fn build(mut self) -> ::std::result::Result<TChannel, String> {
        let peers = PeersPool::default();
        self.peers_pool(Arc::new(peers));
        self.build_internal()
    }
}

impl TChannel {
    pub fn make_subchannel(&mut self, service_name: &str) -> SubChannel {
        SubChannel {
            service_name: service_name.to_string(),
            handlers: HashMap::new(),
            peers_pool: self.peers_pool.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SubChannel {
    service_name: String,
    handlers: HashMap<String, Box<RequestHandler>>,
    peers_pool: Arc<PeersPool>,
}

impl SubChannel {
    pub fn register<HANDLER>(&mut self, handler_name: &str, handler: HANDLER) -> &Self {
        //TODO
        self
    }

    async fn send<REQ: Request, RES: Response, BUILDER: ResponseBuilder<RES>>(
        &self,
        request: REQ,
        responseBuilder: BUILDER,
        host: SocketAddr,
        port: u16,
    ) -> Result<RES> {
        // let peer = self.peers_pool.get_or_add(host).await?;

        Ok(responseBuilder.build())
    }
}

#[derive(Debug, Default, Builder, Clone)]
pub struct ConnectionOptions {}

#[derive(Debug, Default)]
pub struct PeersPool {
    peers: RwLock<HashMap<SocketAddr, Arc<Peer>>>,
}

impl PeersPool {
    pub async fn get_or_add(&self, addr: SocketAddr) -> Result<Arc<Peer>> {
        let peers = self.peers.read().unwrap(); //TODO handle panic
        match peers.get(&addr) {
            Some(peer) => Ok(peer.clone()),
            None => self.add_peer(addr).await,
        }
    }

    async fn add_peer(&self, addr: SocketAddr) -> Result<Arc<Peer>> {
        let mut peers = self.peers.write().unwrap(); //TODO handle panic
        match peers.get(&addr) {
            Some(peer) => Ok(peer.clone()),
            None => {
                let socket = TcpStream::connect(addr).await?;
                let peer = Arc::new(Peer::from(socket));
                peers.insert(addr, peer.clone());
                Ok(peer)
            }
        }
    }

    async fn connect<T: ToSocketAddrs>(&self, addr: T) -> crate::Result<Connection> {
        let socket = TcpStream::connect(addr).await?;
        let connection = Connection::new(socket);
        return Ok(connection);
    }
}

#[derive(Debug)]
pub struct Peer {
    // frames: Framed<TcpStream, TFrameCodec>,
}

impl From<TcpStream> for Peer {
    fn from(stream: TcpStream) -> Self {
        Peer {
            // frames: Framed::new(stream, TFrameCodec)
        }
    }
}
