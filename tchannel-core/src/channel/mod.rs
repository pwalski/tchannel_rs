pub mod messages;

use crate::channel::messages::{Request, Response};
use crate::connection::Connection;
use crate::frame::TFrame;
use crate::handlers::RequestHandler;
use crate::transport::TFrameCodec;
use crate::TChannelError;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::net::ToSocketAddrs;
use tokio::sync::RwLock;
use tokio_util::codec::Framed;

#[derive(Default, Builder)]
#[builder(pattern = "owned")]
#[builder(build_fn(name = "build_internal"))]
pub struct TChannel {
    subchannels: HashMap<String, SubChannel>,
    connection_options: ConnectionOptions,
    #[builder(field(private))]
    pub(super) peers_pool: Arc<PeersPool>,
}

impl TChannelBuilder {
    pub fn build(self) -> ::std::result::Result<TChannel, String> {
        let peers = PeersPool::default();
        self.peers_pool(Arc::new(peers)).build_internal()
    }
}

impl TChannel {
    pub fn make_subchannel(
        &mut self,
        service_name: &str,
    ) -> std::result::Result<SubChannel, String> {
        SubChannelBuilder::default()
            .service_name(service_name.to_string())
            .peers_pool(self.peers_pool.clone())
            .build()
    }
}

#[derive(Debug, Builder)]
#[builder(pattern = "owned")]
pub struct SubChannel {
    service_name: String,
    next_message_id: AtomicI32,
    handlers: HashMap<String, Box<RequestHandler>>,
    peers_pool: Arc<PeersPool>,
}

impl SubChannel {
    pub fn register<HANDLER>(&mut self, handler_name: &str, handler: HANDLER) -> &Self {
        //TODO
        self
    }

    async fn send<REQ: Request, RES: Response + TryFrom<TFrame>>(
        &self,
        request: REQ,
        host: SocketAddr,
        port: u16,
    ) -> Result<RES, crate::TChannelError> {
        let peer = self.peers_pool.get_or_add(host).await;
        let messsage_id = self.next_message_id();
        // RES::try_from()
        unimplemented!()
    }

    fn next_message_id(&self) -> i32 {
        self.next_message_id.fetch_add(1, Ordering::Relaxed)
    }
}

#[derive(Debug, Default, Builder, Clone)]
pub struct ConnectionOptions {}

#[derive(Debug, Default)]
pub struct PeersPool {
    peers: RwLock<HashMap<SocketAddr, Arc<Peer>>>,
}

impl PeersPool {
    pub async fn get_or_add(&self, addr: SocketAddr) -> Result<Arc<Peer>, TChannelError> {
        let peers = self.peers.read().await; //TODO handle panic
        match peers.get(&addr) {
            Some(peer) => Ok(peer.clone()),
            None => self.add_peer(addr).await,
        }
    }

    async fn add_peer(&self, addr: SocketAddr) -> Result<Arc<Peer>, TChannelError> {
        let mut peers = self.peers.write().await; //TODO handle panic
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

    async fn connect<T: ToSocketAddrs>(&self, addr: T) -> Result<Connection, TChannelError> {
        let socket = TcpStream::connect(addr).await?;
        let connection = Connection::new(socket);
        return Ok(connection);
    }
}

#[derive(Debug)]
pub struct Peer {
    frame_codec: Framed<TcpStream, TFrameCodec>,
    // method returning "future" returning response
    // keep in mind it will not happen immidiatelly after request
}

impl From<TcpStream> for Peer {
    fn from(stream: TcpStream) -> Self {
        Peer {
            frame_codec: Framed::new(stream, TFrameCodec),
        }
    }
}
