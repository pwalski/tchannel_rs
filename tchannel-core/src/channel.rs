use crate::handlers::RequestHandler;
use crate::messages::{Request, Response};
use crate::Result;
use crate::connection::Connection;

use std::collections::HashMap;
use std::net::SocketAddr;
use tokio::net::ToSocketAddrs;
use tokio::net::TcpStream;
use crate::frame::Type::Error;

use std::rc::Rc;
use std::ops::Deref;
use tokio_util::codec::Framed;
use crate::transport::TFrameCodec;
use std::cell::{Cell, RefCell};
use std::borrow::BorrowMut;
use std::sync::{Arc, Mutex, RwLock};


#[derive(Default, Builder)]
#[builder(pattern = "mutable")]
#[builder(build_fn(name = "build_internal"))]
pub struct TChannel {
    subchannels: HashMap<String, SubChannel>,
    connection_options: ConnectionOptions,
    #[builder(field(private))]
    peers_pool: Arc<PeersPool>,
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
    pub fn register<HANDLER: >(
        &mut self,
        handler_name: &str,
        handler: HANDLER,
    ) -> &Self {
        //TODO
        self
    }

    pub async fn send<REQ: Request, RES: Response>(&mut self, request: REQ, host: SocketAddr, port: u16) -> RES {
        let peer = self.peers_pool.get_or_add(host);
        // let peer = self.peers_pool.get_or_add(host);
        // match self.peers_pool.try_borrow_mut() {
        //     Ok(pool) => pool.print_shit()
        // }
        println!("done");
        // set transport header
        unimplemented!()
    }
}

#[derive(Debug, Default, Builder, Clone)]
pub struct ConnectionOptions {}

#[derive(Debug, Default)]
pub struct PeersPool {
    peers: RwLock<HashMap<SocketAddr, Rc<Peer>>>,
}

impl PeersPool {

    pub async fn get_or_add(&self, addr: SocketAddr) -> Result<Rc<Peer>> {
        let peers = self.peers.read().unwrap(); //TODO handle panic
        match peers.get(&addr) {
            Some(peer) => Ok(peer.clone()),
            None => add(addr)
        }
        // match self.peers.read() {
        //     Ok(peers) => {}
        //     _ => {}
        // }

        // let peers = self.peers.
        //
        // if let Some(peer) = self.peers.get(&addr) {
        //     return Ok(peer.clone());
        // }
    }

    async fn add(&self, addr: SocketAddr) -> Result<Rc<Peer>> {
        let mut peers = self.peers.write().unwrap(); //TODO handle panic
        match peers.get(&addr) {
            Some(peer) => Ok(peer.clone()),
            None => {
                let stream = connect(addr)?;
                let peer = Rc::new(Peer::from(stream));
                peers.insert(addr, peer);
                Ok(peer.clone())
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
    frames: Framed<TcpStream, TFrameCodec>,
}

impl From<TcpStream> for Peer {
    fn from(stream: TcpStream) -> Self {
        Peer { frames: Framed::new(stream, TFrameCodec) }
    }
}
