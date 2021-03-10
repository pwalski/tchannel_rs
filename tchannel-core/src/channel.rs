use crate::handlers::RequestHandler;
use crate::messages::{Request, Response};
use crate::Result;
use crate::connection::Connection;

use std::collections::HashMap;

use tokio::net::ToSocketAddrs;
use tokio::net::TcpStream;


#[derive(Debug, Default, Builder)]
#[builder(pattern = "owned")]
#[builder(build_fn(name = "build_internal"))]
pub struct TChannel {
    subchannels: HashMap<String, SubChannel>,
    connectionOptions: ConnectionOptions,
    peers: Peers,
}

impl TChannelBuilder {
    pub fn build(mut self) -> ::std::result::Result<TChannel, String> {
        self.peers = Some(Peers{});
        self.build_internal()
    }
}

impl TChannel {
    pub fn makeSubchannel(&mut self, service_name: &str) -> SubChannel {
        SubChannel {
            service_name: service_name.to_string(),
            handlers: HashMap::new(),
        }
    }
}

#[derive(Debug)]
pub struct SubChannel {
    service_name: String,
    handlers: HashMap<String, Box<dyn RequestHandler>>,
}

impl SubChannel {
    pub fn register<HANDLER: RequestHandler>(
        &mut self,
        handler_name: &str,
        handler: HANDLER,
    ) -> &Self {
        //TODO
        self
    }

    pub fn send(&self, request: &dyn Request, host: &str, port: u16) -> Result<Box<dyn Response>> {
        self.handlers.get("").unwrap().handle(request)
    }
}

#[derive(Debug, Default, Builder)]
pub struct ConnectionOptions {}

#[derive(Debug, Default)]
pub struct Peers {}

impl Peers {
    pub async fn connect<T: ToSocketAddrs>(addr: T) -> crate::Result<Connection> {
        let socket = TcpStream::connect(addr).await?;
        let connection = Connection::new(socket);
        return Ok(connection);
    }
}

pub struct Peer {}
