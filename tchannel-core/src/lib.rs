pub mod messages;
pub mod connection;
pub mod frame;
pub mod codec;

use messages::thrift::*;

use connection::*;

use tokio::net::TcpStream;

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T> = std::result::Result<T, Error>;

pub struct Channel {
    id: u32,
    connectionOptions: ConnectionOptions,
    peers: PeerManager
}

impl Channel {
    pub fn new(name: String) -> Result<Channel> {
        let peers = PeerManager{};
        let connectionOptions = ConnectionOptions{};
        let channel = Channel{id: 1, connectionOptions, peers};
        Ok(channel)
    }

    // Make subchannel for given service name.
    pub fn makeSubchannel(&mut self, name: String) -> SubChannel {
        SubChannel { service: String::from("my_service") }
    }
}

pub struct SubChannel {
    pub service: String
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

pub struct ConnectionOptions {
}

use tokio::net::ToSocketAddrs;

pub struct PeerManager {
}

impl PeerManager {
    pub async fn connect<T: ToSocketAddrs>(addr: T) -> crate::Result<Connection> {
        let socket = TcpStream::connect(addr).await?;
        let connection = Connection::new(socket);
        return Ok(connection);
    }
}

// impl PeerManager {
//     pub async fn connect<T: ToSocketAddrs>(addr: T) -> crate::Result<Client> {
//         // The `addr` argument is passed directly to `TcpStream::connect`. This
//         // performs any asynchronous DNS lookup and attempts to establish the TCP
//         // connection. An error at either step returns an error, which is then
//         // bubbled up to the caller of `mini_redis` connect.
//         let socket = TcpStream::connect(addr).await?;
//
//         // Initialize the connection state. This allocates read/write buffers to
//         // perform redis protocol frame parsing.
//         let connection = Connection::new(socket);
//
//         Ok(Client { connection })
//     }
// }

pub use connection::Connection;


pub struct Peer {
}

use messages::{Response, Request};

pub trait RequestHandler<REQ: Request, RES: Response> {
    fn handle(response: REQ) -> RES;
}

#[cfg(test)]
mod tests {
    #[test]
    fn functional_test() {
        // - new channel for client

        // - register handler to channel

        // - add peer using addr string
        // - create subchannel using serviceName string
        // - create client using channel, subchannel, clientoptions, and serviceName
        assert_eq!(2 + 2, 4);
    }
}
