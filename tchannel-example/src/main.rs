use std::collections::HashMap;
use tchannel::channel::messages::thrift::*;
use tchannel::channel::*;
use tchannel::transport::*;

use tokio::net::{TcpStream, ToSocketAddrs};

use tokio_util::codec::Framed;

use tchannel::frame::{InitFrame, Type};

use tchannel::Result;

use futures::SinkExt;
use tchannel::channel::messages::TransportHeader;
use tchannel::channel::TChannel;
use tokio_stream::StreamExt;

#[tokio::main]
pub async fn main() -> Result<()> {
    let mut channel = TChannelBuilder::default().build().unwrap();
    let subChannel = channel.make_subchannel("keyvalue-service");
    let mut transportHeaders = HashMap::new();
    transportHeaders.insert(TransportHeader::CALLER_NAME_KEY, "keyvalue-service");

    let headers: HashMap<String, String> = HashMap::new();
    let initFrame = InitFrame::new(0, Type::InitRequest, headers);

    // let mut connection = connection().await; // ???
    // println!("writing frame");
    // connection.write_iframe(&initFrame).await;
    // println!("wrote frame");
    //
    // println!("reading frame");
    // connection.read_frame().await;
    // println!("got frame");
    //
    // println!("reading frame");
    //
    // // let request = ThriftRequest { value: String::from(""), transportHeaders: transportHeaders };
    // // let response = subChannel.send(request, String::from("localhost"), 8888);

    // Codec

    let stream = connect().await.unwrap();
    let mut transport = Framed::new(stream, TFrameCodec);
    let sent = transport.send(initFrame.frame).await;
    println!("Sent: {:?}", sent);

    while let Some(request) = transport.next().await {
        match request {
            Ok(request) => {
                println!("Incoming frame: {:?}", request)
            }
            Err(e) => return Err(e.into()),
        }
    }

    Ok(())
}

// async fn connection() -> Connection {
//     let addr = String::from("192.168.50.172:8888");
//     let socket = connect();
//     return Connection::new(socket);
// }

pub type TResult<T> = std::result::Result<T, std::io::Error>;

async fn connect() -> TResult<TcpStream> {
    let addr = String::from("192.168.50.172:8888");
    TcpStream::connect(addr).await
}
