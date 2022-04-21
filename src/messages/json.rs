use crate::errors::CodecError;
use crate::frames::headers::ArgSchemeValue;
use crate::handler::HandlerResult;
use crate::messages::args::{MessageArgs, MessageWithArgs};
use crate::messages::{Message, MessageChannel};
use crate::subchannel::SubChannel;
use bytes::{Buf, Bytes};
use futures::Future;
#[cfg(feature = "json")]
use serde_json::ser::to_string;
use std::collections::VecDeque;
use std::fmt::{Display, Formatter};
use std::net::ToSocketAddrs;
use std::pin::Pin;
use std::string::FromUtf8Error;

#[cfg(feature = "json")]
use serde_json::{Map, Value};

/// `RawMessage` is intended for any custom encodings you want to do that are not part of TChannel.
///
///  Consider using the thrift, sthrift, json or http encodings before using it.
#[derive(Default, Debug, Clone, Builder, Getters, new)]
#[builder(pattern = "owned")]
pub struct JsonMessage {
    //arg1
    #[get = "pub"]
    method_name: String,
    //arg2
    #[get = "pub"]
    headers: Map<String, Value>,
    //arg3
    #[get = "pub"]
    body: Map<String, Value>,
}

impl Display for JsonMessage {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Method name: '{}', Headers: '{:?}', Body: '{:?}'",
            self.method_name, self.headers, self.body
        )
    }
}

impl TryFrom<MessageArgs> for JsonMessage {
    type Error = CodecError;

    fn try_from(args: MessageArgs) -> Result<Self, Self::Error> {
        let mut deq_args = VecDeque::from(args.args);
        if args.arg_scheme != ArgSchemeValue::Json {
            return Err(CodecError::Error(format!(
                "Wrong arg scheme {:?}",
                args.arg_scheme
            )));
        }
        Ok(JsonMessage::new(
            bytes_to_string(deq_args.pop_front())?,
            bytes_to_json(deq_args.pop_front())?,
            bytes_to_json(deq_args.pop_front())?,
        ))
    }
}

impl TryInto<MessageArgs> for JsonMessage {
    type Error = CodecError;

    fn try_into(self) -> Result<MessageArgs, Self::Error> {
        let arg_scheme = JsonMessage::args_scheme();
        let args = Vec::from([
            self.method_name.into(),
            to_string(&self.headers)?.into(),
            to_string(&self.body)?.into(),
        ]);
        Ok(MessageArgs::new(arg_scheme, args))
    }
}

fn bytes_to_string(arg: Option<Bytes>) -> Result<String, FromUtf8Error> {
    arg.map_or_else(
        || Ok(String::new()),
        |b| String::from_utf8(Vec::from(b.chunk())),
    )
}

fn bytes_to_json(arg: Option<Bytes>) -> Result<Map<String, Value>, CodecError> {
    let arg = arg.unwrap_or_default();
    Ok(serde_json::from_slice(arg.as_ref())?)
}

impl Message for JsonMessage {}

impl MessageWithArgs for JsonMessage {
    fn args_scheme() -> ArgSchemeValue {
        ArgSchemeValue::Json
    }
}

impl MessageChannel<JsonMessage, JsonMessage> for SubChannel {
    fn send<'a, ADDR: ToSocketAddrs + Send + 'a>(
        &'a self,
        request: JsonMessage,
        host: ADDR,
    ) -> Pin<Box<dyn Future<Output = HandlerResult<JsonMessage>> + Send + '_>> {
        Box::pin(self.send(request, host))
    }
}
