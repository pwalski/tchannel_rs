use crate::handlers::RequestHandler;
use crate::messages::{Request, Response};
use crate::Result;
use crate::connection::Connection;

use std::collections::HashMap;
use std::net::SocketAddr;
use tokio::net::ToSocketAddrs;
use tokio::net::TcpStream;
use crate::frame::Type::Error;

use tokio::net::lookup_host;
use std::rc::Rc;
use std::ops::Deref;


#[derive(Default, Builder)]
#[builder(pattern = "mutable")]
#[builder(build_fn(name = "build_internal"))]
pub struct TChannel {
    subchannels: HashMap<String, SubChannel>,
    connectionOptions: ConnectionOptions,
    #[builder(field(private))]
    peers: PeersPool,
}

impl TChannelBuilder {
    pub fn build(mut self) -> ::std::result::Result<TChannel, String> {
        self.peers(PeersPool::default());
        self.build_internal()
    }
}

impl TChannel {
    pub fn make_subchannel(&mut self, service_name: &str) -> SubChannel {
        SubChannel {
            service_name: service_name.to_string(),
            handlers: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SubChannel {
    service_name: String,
    handlers: HashMap<String, Box<RequestHandler>>,
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

    pub fn send<REQ: Request, RES: Response>(&self, request: REQ, host: &str, port: u16) -> RES {
        // set transport header
        unimplemented!()
    }
}

#[derive(Debug, Default, Builder, Clone)]
pub struct ConnectionOptions {}

#[derive(Debug, Default, Clone)]
pub struct PeersPool {
    peers: HashMap<SocketAddr, Rc<Peer>>
}

impl PeersPool {
    pub fn get_or_add<ADDR: ToSocketAddrs>(&mut self, addr: SocketAddr) -> Rc<Peer> {
        if let Some(peer) = self.peers.get(&addr) {
            return peer.clone();
        }
        self.add(addr)
    }

    fn add(&mut self, addr: SocketAddr) -> Rc<Peer> {
        let peer = Rc::new(Peer { address: addr });
        self.peers.insert(addr,peer.clone());
        return peer;
    }

    pub async fn connect<T: ToSocketAddrs>(addr: T) -> crate::Result<Connection> {
        let socket = TcpStream::connect(addr).await?;
        let connection = Connection::new(socket);
        return Ok(connection);
    }
}

#[derive(Debug, Clone)]
pub struct Peer {
    address: SocketAddr
}
