use crate::errors::CodecError;
use crate::frames::headers::ArgSchemeValue;
use crate::handler::Response;
use crate::messages::{Message, MessageArgs, MessageChannel};
use crate::subchannel::SubChannel;
use bytes::{Buf, Bytes};
use futures::Future;
use std::collections::VecDeque;
use std::convert::{TryFrom, TryInto};
use std::net::SocketAddr;
use std::pin::Pin;
use std::string::FromUtf8Error;

#[derive(Default, Debug, Builder, Getters, MutGetters, new)]
#[builder(pattern = "owned")]
pub struct RawMessage {
    //arg1
    #[get = "pub"]
    endpoint: String,
    //arg2
    #[get = "pub"]
    header: String,
    //arg3
    #[get = "pub"]
    #[get_mut = "pub"]
    body: Bytes,
}

impl RawMessage {}

impl TryFrom<MessageArgs> for RawMessage {
    type Error = CodecError;

    fn try_from(args: MessageArgs) -> Result<Self, Self::Error> {
        let mut deq_args = VecDeque::from(args.args);
        if args.arg_scheme != ArgSchemeValue::Raw {
            return Err(CodecError::Error(format!(
                "Wrong arg scheme {:?}",
                args.arg_scheme
            )));
        }
        Ok(RawMessage::new(
            bytes_to_string(deq_args.pop_front())?,
            bytes_to_string(deq_args.pop_front())?,
            deq_args.pop_front().unwrap_or_else(Bytes::new),
        ))
    }
}

impl TryInto<MessageArgs> for RawMessage {
    type Error = CodecError;

    fn try_into(self) -> Result<MessageArgs, Self::Error> {
        Ok(MessageArgs::new(
            RawMessage::args_scheme(),
            Vec::from([self.endpoint.into(), self.header.into(), self.body]),
        ))
    }
}

impl Message for RawMessage {
    fn args_scheme() -> ArgSchemeValue {
        ArgSchemeValue::Raw
    }
}

fn bytes_to_string(arg: Option<Bytes>) -> Result<String, FromUtf8Error> {
    arg.map_or_else(
        || Ok(String::new()),
        |b| String::from_utf8(Vec::from(b.chunk())),
    )
}

impl MessageChannel for SubChannel {
    type REQ = RawMessage;
    type RES = RawMessage;

    fn send(
        &self,
        request: Self::REQ,
        host: SocketAddr,
    ) -> Pin<Box<dyn Future<Output = Response<Self::RES>> + Send + '_>> {
        Box::pin(self.send(request, host))
    }
}