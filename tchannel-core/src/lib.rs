extern crate num;

#[macro_use]
extern crate getset;

#[macro_use]
extern crate num_derive;

#[macro_use]
extern crate derive_builder;

pub mod channel;
pub mod codec;
pub mod connection;
pub mod frame;
pub mod handlers;
pub mod messages;
pub mod transport;

use messages::thrift::*;

use connection::*;

use tokio::net::TcpStream;

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T> = std::result::Result<T, Error>;

pub struct Channel {
    id: u32,
    connectionOptions: ConnectionOptions,
    peers: Peers,
}

impl Channel {
    pub fn new(name: String) -> Result<Channel> {
        let peers = Peers {};
        let connectionOptions = ConnectionOptions {};
        let channel = Channel {
            id: 1,
            connectionOptions,
            peers,
        };
        Ok(channel)
    }

    // Make subchannel for given service name.
    pub fn makeSubchannel(&mut self, name: String) -> SubChannel {
        SubChannel {
            service: String::from("my_service"),
        }
    }
}

pub struct SubChannel {
    pub service: String,
}

impl SubChannel {
    // send thrift request return thrift response

    pub fn register(&mut self) -> &mut SubChannel {
        self
    }

    pub fn send(&self, message: ThriftRequest, host: String, port: u16) -> Result<ThriftResponse> {
        Ok(ThriftResponse {})
    }
}

pub struct ConnectionOptions {}

use tokio::net::ToSocketAddrs;

pub struct Peers {}

impl Peers {
    pub async fn connect<T: ToSocketAddrs>(addr: T) -> crate::Result<Connection> {
        let socket = TcpStream::connect(addr).await?;
        let connection = Connection::new(socket);
        return Ok(connection);
    }
}

pub use connection::Connection;

pub struct Peer {}
