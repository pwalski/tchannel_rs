use std::collections::HashMap;

use bytes::{BufMut, BytesMut};
use futures::SinkExt;
use tokio::net::TcpStream;
use tokio_stream::StreamExt;
use tokio_util::codec::Framed;

use tchannel::channel::frames::headers::TransportHeader;
use tchannel::channel::frames::payloads::*;
use tchannel::channel::frames::*;
use tchannel::channel::frames::{TFrame, TFrameIdCodec, Type};
use tchannel::error::Error;
use tchannel::Error;

// pub struct InitFrame {
//     // pub because generics suc
//     pub frame: TFrame,
// }

// impl InitFrame {
//     const VERSION: u16 = 2;
//
//     pub fn new(id: u32, frame_type: Type, headers: HashMap<String, String>) -> Self {
//         let mut bytes = BytesMut::new();
//         bytes.put_u16(InitFrame::VERSION);
//         encode_headers(&headers, &mut bytes);
//         let frame = TFrameBuilder::default()
//             .id(id)
//             .frame_type(frame_type)
//             .payload(bytes.freeze())
//             .build()
//             .unwrap(); //TODO
//         return InitFrame { frame };
//     }
// }

#[tokio::main]
pub async fn main() -> Result<(), Error> {
    env_logger::init();
    // let mut channel = TChannelBuilder::default().build().unwrap();
    // let sub_channel = channel.make_subchannel("keyvalue-service");
    let mut transport_headers = HashMap::new();
    transport_headers.insert(TransportHeader::CallerNameKey, "keyvalue-service");

    let headers: HashMap<String, String> = HashMap::new();
    let init_frame = Init::new(PROTOCOL_VERSION, headers);
    let mut bytes = BytesMut::new();
    // let id = 1;
    init_frame.encode(&mut bytes);
    let frame = TFrame::new(Type::InitRequest, bytes.freeze());
    let frame_id = TFrameId::new(1, frame);

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
    let mut transport = Framed::new(stream, TFrameIdCodec::default());
    let sent = transport.send(frame_id).await;
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
