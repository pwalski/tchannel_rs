use tchannel::{Channel, Connection};
use tchannel::messages::thrift::*;
use tchannel::messages::headers;
use std::collections::HashMap;

use tokio::net::{TcpStream, ToSocketAddrs};

use tchannel::frame::{InitFrame, Type};

use tchannel::Result;

#[tokio::main]
pub async fn main() -> Result<()> {
    let mut channel = Channel::new(String::from("keyvalue-client")).unwrap();
    let subChannel = channel.makeSubchannel(String::from("keyvalue-service"));
    let mut transportHeaders = HashMap::new();
    transportHeaders.insert(headers::CALLER_NAME_KEY, & subChannel.service);

    let headers: HashMap<String, String> = HashMap::new();
    let initFrame = InitFrame::new(0, Type::InitRequest, headers);

    let mut connection = connect().await; // ???
    println!("writing frame");
    connection.write_iframe(&initFrame).await;
    println!("wrote frame");

    println!("reading frame");
    connection.read_frame().await;
    println!("got frame");

    println!("reading frame");


    // let request = ThriftRequest { value: String::from(""), transportHeaders: transportHeaders };
    // let response = subChannel.send(request, String::from("localhost"), 8888);

    Ok(())
}

async fn connect() -> Connection {
    let addr = String::from("192.168.50.172:8888");
    let socket = TcpStream::connect(addr).await.unwrap();
    return Connection::new(socket);
}
