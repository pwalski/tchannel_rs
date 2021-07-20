use crate::channel::frames::headers::TransportHeader::CallerNameKey;
use crate::channel::frames::headers::{ArgSchemeValue, TransportHeader};
use crate::channel::frames::payloads::Codec;
use crate::channel::frames::payloads::{
    CallArgs, CallContinue, CallFieldsEncoded, CallRequestFields, ChecksumType, Flags, TraceFlags,
    Tracing, ARG_LEN_LEN,
};
use crate::channel::frames::{TFrame, TFrameStream, Type, FRAME_HEADER_LENGTH, FRAME_MAX_LENGTH};
use crate::channel::messages::fragmenting::FragmentationStatus::{CompleteAtTheEnd, Incomplete};
use crate::channel::messages::Request;
use crate::TChannelError;
use bytes::Buf;
use bytes::{Bytes, BytesMut};
use std::collections::{HashMap, VecDeque};

#[derive(Debug, new)]
pub struct Fragmenter<REQ: Request> {
    request: REQ,
    service_name: String,
}

impl<REQ: Request> Fragmenter<REQ> {
    pub async fn create_frames(self) -> Result<TFrameStream, TChannelError> {
        let mut args = self.request.args();
        args.reverse();

        let mut request_fields = create_request_fields_bytes(REQ::arg_scheme(), self.service_name)?;
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

fn calculate_payload_limit(fields_len: usize) -> usize {
    //64KiB max frame size - header size -1 (flag) - serialized fields size
    FRAME_MAX_LENGTH as usize - FRAME_HEADER_LENGTH as usize - 1 - fields_len
}

fn create_tracing() -> Tracing {
    Tracing::new(0, 0, 0, TraceFlags::NONE)
}

fn calculate_checksum(args: &VecDeque<Option<Bytes>>, csum_type: ChecksumType) -> Option<u32> {
    match csum_type {
        ChecksumType::None => None,
        other => todo!("Unsupported checksum type {:?}", other),
    }
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

fn get_frame_flag(remaining_args: &Vec<Bytes>) -> Flags {
    if remaining_args.is_empty() {
        Flags::NONE
    } else {
        Flags::MORE_FRAGMENTS_FOLLOW
    }
}
