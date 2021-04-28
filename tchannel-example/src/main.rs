use futures::SinkExt;
use std::collections::HashMap;
use tchannel::channel::frames::{TFrameCodec, Type};
use tchannel::channel::messages::TransportHeader;
use tchannel::frame::InitFrame;
use tchannel::Error;
use tokio::net::TcpStream;
use tokio_stream::StreamExt;
use tokio_util::codec::Framed;

#[tokio::main]
pub async fn main() -> Result<(), Error> {
    // let mut channel = TChannelBuilder::default().build().unwrap();
    // let sub_channel = channel.make_subchannel("keyvalue-service");
    let mut transport_headers = HashMap::new();
    transport_headers.insert(TransportHeader::CallerNameKey, "keyvalue-service");

    let headers: HashMap<String, String> = HashMap::new();
    let init_frame = InitFrame::new(0, Type::InitRequest, headers);

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
    // let request = ThriftRequest { value: String::from(""), transportHeaders: transportHeaders };
    // let response = sub_channel.send(request, String::from("localhost"), 8888);

    // Codec

    let stream = connect().await.unwrap();
    let mut transport = Framed::new(stream, TFrameCodec::default());
    let sent = transport.send(init_frame.frame).await;
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
