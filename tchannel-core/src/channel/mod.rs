pub mod connection;
pub mod frames;
pub mod messages;

use crate::channel::connection::{
    ConnectionOptions, ConnectionPools, ConnectionPoolsBuilder, FrameInput, FrameOutput,
};
use crate::channel::frames::headers::TransportHeader::CallerNameKey;
use crate::channel::frames::headers::{ArgSchemeValue, TransportHeader};
use crate::channel::frames::payloads::ResponseCode;
use crate::channel::frames::payloads::{
    CallArgs, CallContinue, CallFieldsEncoded, CallRequest, CallRequestFields, CallResponse,
    ChecksumType, Codec, Flags, TraceFlags, Tracing, ARG_LEN_LEN,
};
use crate::channel::frames::Type::CallResponseContinue;
use crate::channel::frames::{
    TFrame, TFrameId, TFrameStream, Type, FRAME_HEADER_LENGTH, FRAME_MAX_LENGTH,
};
use crate::channel::messages::{Message, Request, Response};
use crate::channel::FragmentationStatus::{CompleteAtTheEnd, Incomplete};
use crate::handlers::RequestHandler;
use crate::{Error, TChannelError};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use futures::channel::oneshot::Sender;
use futures::join;
use futures::task::Poll;
use futures::StreamExt;
use futures::{future, TryStreamExt};
use futures::{FutureExt, Stream};
use log::{debug, error, info, warn};
use std::collections::{HashMap, VecDeque};
use std::convert::{Infallible, TryFrom, TryInto};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::net::ToSocketAddrs;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio_util::codec::{Decoder, Encoder, Framed};

pub struct TChannel {
    subchannels: RwLock<HashMap<String, Arc<SubChannel>>>,
    connection_pools: Arc<ConnectionPools>,
}

impl TChannel {
    pub fn new(connection_options: ConnectionOptions) -> Result<Self, TChannelError> {
        let connection_pools = ConnectionPoolsBuilder::default()
            .connection_options(connection_options)
            .build()?;
        Ok(TChannel {
            subchannels: RwLock::new(HashMap::new()),
            connection_pools: Arc::new(connection_pools),
        })
    }

    pub async fn subchannel(
        &mut self,
        service_name: String,
    ) -> Result<Arc<SubChannel>, TChannelError> {
        if let Some(subchannel) = self.subchannels.read().await.get(&service_name) {
            return Ok(subchannel.clone());
        }
        self.make_subchannel(service_name).await
    }

    async fn make_subchannel(
        &self,
        service_name: String,
    ) -> Result<Arc<SubChannel>, TChannelError> {
        let mut subchannels = self.subchannels.write().await;
        match subchannels.get(&service_name) {
            Some(subchannel) => Ok(subchannel.clone()),
            None => {
                debug!("Creating subchannel {}", service_name);
                let subchannel = Arc::new(
                    SubChannelBuilder::default()
                        .service_name(service_name.to_owned())
                        .connection_pools(self.connection_pools.clone())
                        .build()?,
                );
                subchannels.insert(service_name, subchannel.clone());
                Ok(subchannel)
            }
        }
    }
}

#[derive(Debug, Builder)]
#[builder(pattern = "owned")]
pub struct SubChannel {
    service_name: String,
    connection_pools: Arc<ConnectionPools>,
    #[builder(setter(skip))]
    handlers: HashMap<String, Box<RequestHandler>>,
}

impl SubChannel {
    pub fn register<HANDLER>(&mut self, handler_name: &str, handler: HANDLER) -> &Self {
        //TODO
        self
    }

    async fn send<REQ: Request, RES: Response>(
        &self,
        request: REQ,
        host: SocketAddr,
    ) -> Result<(ResponseCode, RES), crate::TChannelError> {
        let (connection_res, frames_res) = join!(
            self.connect(host),
            create_frames(request, self.service_name.clone())
        );
        let (frames_out, mut frames_in) = connection_res?;
        send_frames(frames_res?, &frames_out).await?;
        let (code, mut args) = read_response(&mut frames_in, RES::arg_scheme()).await?;
        frames_out.close(); //TODO ugly
        debug!("Received args {:?}", args);
        Ok((code, RES::try_from(args)?))
    }

    async fn connect(&self, host: SocketAddr) -> Result<(FrameOutput, FrameInput), TChannelError> {
        let pool = self.connection_pools.get(host).await?;
        let connection = pool.get().await?;
        Ok(connection.new_message_io().await)
    }
}

// TODO separate fragmentation related code

async fn send_frames(frames: TFrameStream, frames_out: &FrameOutput) -> Result<(), TChannelError> {
    debug!("Sending frames");
    frames
        .then(|frame| frames_out.send(frame))
        .inspect_err(|err| error!("Failed to send frame {:?}", err))
        .try_for_each(|res| future::ready(Ok(())))
        .await
}

async fn create_frames<REQ: Request>(
    request: REQ,
    service_name: String,
) -> Result<TFrameStream, TChannelError> {
    let mut args = request.args();
    args.reverse();

    let mut request_fields = create_request_fields_bytes(REQ::arg_scheme(), service_name)?;
    let payload_limit = calculate_payload_limit(request_fields.len());
    let frame_args = create_frame_args(&mut args, payload_limit);
    let flag = get_frame_flag(&args);

    let mut call_frames = Vec::new();
    let call_request = CallFieldsEncoded::new(flag, request_fields, frame_args);
    debug!("Creating call request {:?}", call_request);
    call_frames.push(TFrame::new(Type::CallRequest, call_request.encode_bytes()?));

    while !args.is_empty() {
        debug!("Creating call continue");
        let payload_limit = calculate_payload_limit(0);
        let frame_args = create_frame_args(&mut args, payload_limit);
        let flag = get_frame_flag(&args);
        let call_continue = CallContinue::new(flag, frame_args);
        call_frames.push(TFrame::new(
            Type::CallRequestContinue,
            call_continue.encode_bytes()?,
        ))
    }
    debug!("Done! {} frames", call_frames.len());
    Ok(Box::pin(futures::stream::iter(call_frames)))
}

fn get_frame_flag(remaining_args: &Vec<Bytes>) -> Flags {
    if remaining_args.is_empty() {
        Flags::NONE
    } else {
        Flags::MORE_FRAGMENTS_FOLLOW
    }
}

fn create_frame_args(args: &mut Vec<Bytes>, payload_limit: usize) -> CallArgs {
    let frame_args = create_frame_args_vec(args, payload_limit);
    let checksum_type = ChecksumType::None;
    let checksum = calculate_checksum(&frame_args, checksum_type);
    CallArgs::new(checksum_type, checksum, frame_args)
}

fn create_frame_args_vec(args: &mut Vec<Bytes>, payload_limit: usize) -> VecDeque<Option<Bytes>> {
    let mut frame_args = VecDeque::with_capacity(3);
    let mut remaining_limit = payload_limit;
    while let Some(mut arg) = args.pop() {
        let (status, frame_arg_bytes) = fragment_arg(&mut arg, remaining_limit);
        frame_arg_bytes.map(|frame_arg| match frame_arg.len() {
            0 => frame_args.push_back(None),
            len => {
                remaining_limit = remaining_limit - (len + ARG_LEN_LEN);
                frame_args.push_back(Some(frame_arg));
            }
        });
        if status == Incomplete {
            args.push(arg);
            break;
        } else if status == CompleteAtTheEnd {
            args.push(Bytes::new());
            break;
        }
    }
    frame_args
}

fn create_request_fields_bytes(
    arg_scheme: ArgSchemeValue,
    service_name: String,
) -> Result<Bytes, TChannelError> {
    let mut bytes = BytesMut::new();
    create_request_fields(arg_scheme, service_name).encode(&mut bytes)?;
    return Ok(Bytes::from(bytes));
}

fn create_request_fields(arg_scheme: ArgSchemeValue, service_name: String) -> CallRequestFields {
    let tracing = create_tracing();
    let headers = create_headers(arg_scheme, service_name.clone()); //TODO get rid of clones
    CallRequestFields::new(60_000, tracing, service_name, headers) //TODO configurable TTL
}

fn calculate_payload_limit(fields_len: usize) -> usize {
    //64KiB max frame size - header size -1 (flag) - serialized fields size
    FRAME_MAX_LENGTH as usize - FRAME_HEADER_LENGTH as usize - 1 - fields_len
}

fn create_headers(arg_scheme: ArgSchemeValue, service_name: String) -> HashMap<String, String> {
    let mut headers = HashMap::new();
    headers.insert(
        TransportHeader::ArgSchemeKey.to_string(),
        arg_scheme.to_string(),
    );
    headers.insert(CallerNameKey.to_string(), service_name);
    return headers;
}

fn create_tracing() -> Tracing {
    Tracing::new(0, 0, 0, TraceFlags::NONE)
}

#[derive(PartialEq)]
enum FragmentationStatus {
    //Completely stored in frame
    Complete,
    //Completely stored in frame with maxed frame capacity
    CompleteAtTheEnd,
    //Partially stored in frame
    Incomplete,
}

fn fragment_arg(arg: &mut Bytes, payload_limit: usize) -> (FragmentationStatus, Option<Bytes>) {
    let src_remaining = arg.remaining();
    if src_remaining == 0 {
        (FragmentationStatus::Complete, None)
    } else if let 0..=ARG_LEN_LEN = payload_limit {
        (FragmentationStatus::Incomplete, None)
    } else if src_remaining + ARG_LEN_LEN > payload_limit {
        let fragment = arg.split_to(payload_limit - ARG_LEN_LEN);
        (FragmentationStatus::Incomplete, Some(fragment))
    } else {
        let arg_bytes = arg.split_to(arg.len());
        if (src_remaining + ARG_LEN_LEN == payload_limit) {
            (FragmentationStatus::CompleteAtTheEnd, Some(arg_bytes))
        } else {
            (FragmentationStatus::Complete, Some(arg_bytes))
        }
    }
}

fn calculate_checksum(args: &VecDeque<Option<Bytes>>, csum_type: ChecksumType) -> Option<u32> {
    match csum_type {
        ChecksumType::None => None,
        other => todo!("Unsupported checksum type {:?}", other),
    }
}

// defragmentation

async fn read_response(
    frame_input: &mut FrameInput,
    arg_scheme: ArgSchemeValue,
) -> Result<(ResponseCode, Vec<Bytes>), TChannelError> {
    let mut args_defragmenter = ArgsDefragmenter::default();
    let (code, flags) =
        read_response_begin(frame_input, arg_scheme, &mut args_defragmenter).await?;
    if !flags.contains(Flags::MORE_FRAGMENTS_FOLLOW) {
        return Ok((code, args_defragmenter.args()));
    }
    read_response_continue(frame_input, &mut args_defragmenter).await?;
    Ok((code, args_defragmenter.args()))
}

async fn read_response_begin(
    frame_input: &mut FrameInput,
    arg_scheme: ArgSchemeValue,
    args_defragmenter: &mut ArgsDefragmenter,
) -> Result<(ResponseCode, Flags), TChannelError> {
    if let Some(frame_id) = frame_input.recv().await {
        debug!("Received response id: {}", frame_id.id());
        let mut frame = frame_id.frame;
        let response = CallResponse::decode(frame.payload_mut())?;
        verify_headers(response.fields().headers(), arg_scheme)?;
        verify_args(response.args).map(|args| args_defragmenter.add(args))?;
        Ok((response.fields.code, response.flags))
    } else {
        Err(TChannelError::Error("Got no response".to_owned()))
    }
}

async fn read_response_continue(
    frame_input: &mut FrameInput,
    args_defragmenter: &mut ArgsDefragmenter,
) -> Result<(), TChannelError> {
    while let Some(frame_id) = frame_input.recv().await {
        let mut frame = frame_id.frame;
        match frame.frame_type() {
            Type::CallResponseContinue => {
                let continuation = CallContinue::decode(frame.payload_mut())?;
                let more_follows = continuation.flags().contains(Flags::MORE_FRAGMENTS_FOLLOW); //TODO workaround for borrow checker
                verify_args(continuation.args).map(|args| args_defragmenter.add(args))?;
                if more_follows {
                    break;
                }
            }
            Type::Error => {
                debug!("Transport error: {:?}", frame);
                return Err(TChannelError::Error(format!(
                    "Transport error: {:?}",
                    frame
                )));
            }
            frame_type => {
                debug!("Unexpected frame: {:?}", frame);
                return Err(TChannelError::Error(format!(
                    "Unexpected frame: {:?}",
                    frame_type
                )));
            }
        };
    }
    Ok(())
}

fn verify_headers(
    headers: &HashMap<String, String>,
    expected_arg_scheme: ArgSchemeValue,
) -> Result<(), TChannelError> {
    if let Some(scheme) = headers.get(TransportHeader::ArgSchemeKey.to_string().as_str()) {
        //TODO ugly
        if !scheme.eq(expected_arg_scheme.to_string().as_str()) {
            return Err(TChannelError::Error(format!(
                "Expected arg scheme '{}' received '{}'",
                expected_arg_scheme.to_string(),
                scheme
            )));
        }
    } else {
        return Err(TChannelError::Error("Missing arg schema arg".to_owned()));
    }
    Ok(())
}

fn verify_args(call_args: CallArgs) -> Result<VecDeque<Option<Bytes>>, TChannelError> {
    match call_args.checksum_type() {
        ChecksumType::None => Ok(call_args.args),
        _ => todo!(),
    }
}

#[derive(Debug, Default)]
struct ArgsDefragmenter {
    args: Vec<BytesMut>,
}

impl ArgsDefragmenter {
    fn add(&mut self, mut frame_args: VecDeque<Option<Bytes>>) {
        if frame_args.is_empty() {
            return;
        } else if let Some(arg) = frame_args.pop_front().unwrap() {
            // First frame arg might be continuation of arg from previous frame
            let mut previous_arg = self.args.pop().ok_or_else(|| BytesMut::new()).unwrap();
            previous_arg.put(arg);
            self.args.push(previous_arg);
        } else {
            self.args.push(BytesMut::new());
        }
        for frame_arg in frame_args {
            match frame_arg {
                None => self.args.push(BytesMut::new()), // begin new arg
                Some(fragment) => {
                    self.args.push(BytesMut::from(fragment.chunk()));
                }
            }
        }
    }

    fn args(&mut self) -> Vec<Bytes> {
        self.args
            .iter_mut()
            .map(|arg_mut| Bytes::from(arg_mut.split_to(arg_mut.len())))
            .collect()
    }
}
