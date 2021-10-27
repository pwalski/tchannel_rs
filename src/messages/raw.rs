use crate::errors::CodecError;
use crate::frames::headers::ArgSchemeValue;
use crate::handler::HandlerResult;
use crate::messages::args::{MessageArgs, MessageWithArgs};
use crate::messages::{Message, MessageChannel};
use crate::subchannel::SubChannel;
use bytes::{Buf, Bytes};
use futures::Future;
use std::collections::VecDeque;
use std::convert::{TryFrom, TryInto};
use std::fmt::{Display, Formatter};
use std::net::ToSocketAddrs;
use std::pin::Pin;
use std::string::FromUtf8Error;

/// `RawMessage` is intended for any custom encodings you want to do that are not part of TChannel.
///
///  Consider using the thrift, sthrift, json or http encodings before using it.
#[derive(Default, Debug, Clone, Builder, Getters, new)]
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
    body: Bytes,
}

impl Display for RawMessage {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Endpoint: '{}', Header: '{}', Body: '{:?}'",
            self.endpoint, self.header, self.body
        )
    }
}

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

impl Message for RawMessage {}

impl MessageWithArgs for RawMessage {
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

    fn send<'a, ADDR: ToSocketAddrs + Send + 'a>(
        &'a self,
        request: Self::REQ,
        host: ADDR,
    ) -> Pin<Box<dyn Future<Output = HandlerResult<Self::RES>> + Send + '_>> {
        Box::pin(self.send(request, host))
    }
}
