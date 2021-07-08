use crate::channel::frames::{TFrame, Type};
use crate::TChannelError;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use num_traits::FromPrimitive;
use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};
use std::fmt::Write;
use std::string::FromUtf8Error;

pub const PROTOCOL_VERSION: u16 = 2;
pub const TRACING_HEADER_LENGTH: u8 = 25;
pub const MAX_FRAME_ARGS: usize = 3;

pub trait Codec: Sized {
    fn encode(self, dst: &mut BytesMut) -> Result<(), TChannelError>;
    fn decode(src: &mut Bytes) -> Result<Self, TChannelError>;

    fn encode_bytes(self) -> Result<Bytes, TChannelError> {
        let mut bytes = BytesMut::new();
        self.encode(&mut bytes);
        Ok(Bytes::from(bytes))
    }
}

#[derive(Copy, Clone, Debug, FromPrimitive, PartialEq, ToPrimitive)]
pub enum ChecksumType {
    None = 0x00,
    // crc-32 (adler-32)
    Crc32 = 0x01,
    // Farmhash Fingerprint32
    Farmhash = 0x02,
    // crc-32C
    Crc32C = 0x03,
}

#[derive(Copy, Clone, Debug, FromPrimitive, ToPrimitive)]
pub enum ResponseCode {
    Ok = 0x00,
    Error = 0x01,
}

#[derive(Copy, Clone, Debug, FromPrimitive, ToPrimitive)]
pub enum ErrorCode {
    // Not a valid value for code. Do not use.
    Invalid = 0x00,
    // No nodes responded successfully within the ttl deadline.
    Timeout = 0x01,
    // Request was cancelled with a cancel message.
    Cancelled = 0x02,
    // 	Node is too busy, this request is safe to retry elsewhere if desired.
    Busy = 0x03,
    // Node declined request for reasons other than load.
    Declined = 0x04,
    // Request resulted in an unexpected error. The request may have been completed before the error.
    UnexpectedError = 0x05,
    // Request args do not match expectations, request will never be satisfied, do not retry.
    BadRequest = 0x06,
    // A network error (e.g. socket error) occurred.
    NetworkError = 0x07,
    // A relay on the network declined to forward the request to an unhealthy node, do not retry.
    Unhealthy = 0x08,
    // Connection will close after this frame. message ID of this frame should be 0xFFFFFFFF.
    FatalProtocolError = 0xff,
}

bitflags! {
    pub struct Flags: u8 {
        const NONE = 0x00; //TODO bit useless
        const MORE_FRAGMENTS_FOLLOW = 0x01;
        const IS_REQUEST_STREAMING = 0x02;
    }
}

bitflags! {
    pub struct TraceFlags: u8 {
        const NONE = 0x00; //TODO bit useless
        const ENABLED = 0x01;
    }
}

#[derive(Debug, new)]
pub struct Tracing {
    span_id: u64,
    parent_id: u64,
    trace_id: u64,
    trace_flags: TraceFlags,
}

impl Codec for Tracing {
    fn encode(mut self, dst: &mut BytesMut) -> Result<(), TChannelError> {
        dst.put_u64(self.span_id);
        dst.put_u64(self.parent_id);
        dst.put_u64(self.trace_id);
        dst.put_u8(self.trace_flags.bits());
        Ok(()) // TODO Ok?
    }

    fn decode(src: &mut Bytes) -> Result<Self, TChannelError> {
        Ok(Tracing::new(
            src.get_u64(),
            src.get_u64(),
            src.get_u64(),
            decode_bitflag(src.get_u8(), TraceFlags::from_bits)?,
        ))
    }
}

#[derive(Debug, Getters, new)]
pub struct Init {
    #[get = "pub"]
    version: u16,
    #[get = "pub"]
    headers: HashMap<String, String>,
}

impl Default for Init {
    fn default() -> Self {
        Init::new(PROTOCOL_VERSION, HashMap::default())
    }
}

impl Codec for Init {
    fn encode(self, dst: &mut BytesMut) -> Result<(), TChannelError> {
        dst.put_u16(self.version);
        encode_headers(self.headers, dst)?;
        Ok(())
    }

    fn decode(src: &mut Bytes) -> Result<Self, TChannelError> {
        Ok(Init::new(src.get_u16(), decode_headers(src)?))
    }
}

//TODO convert to builder and verify args length?
#[derive(Debug, new)]
pub struct CallArgs {
    // csumtype:1
    checksum_type: ChecksumType,
    // (csum:4){0,1}
    checksum: Option<u32>,
    // arg1~2 arg2~2 arg3~2
    args: Vec<Option<Bytes>>,
}

impl Codec for CallArgs {
    fn encode(mut self, dst: &mut BytesMut) -> Result<(), TChannelError> {
        dst.put_u8(self.checksum_type as u8);
        encode_checksum(self.checksum_type, self.checksum, dst)?;
        encode_args(self.args, dst)?;
        Ok(())
    }

    fn decode(src: &mut Bytes) -> Result<Self, TChannelError> {
        let (checksum_type, checksum) = decode_checksum(src)?;
        Ok(CallArgs::new(
            checksum_type,
            checksum,
            decode_args(src)?, //arg3
        ))
    }
}

#[derive(Debug, new)]
pub struct CallFieldsEncoded {
    // flags:1
    flags: Flags,
    // ttl, tracing, service name, headers
    fields: Bytes,
    // checksum type, checksum, args
    args: CallArgs,
}

impl Codec for CallFieldsEncoded {
    fn encode(self, dst: &mut BytesMut) -> Result<(), TChannelError> {
        dst.put_u8(self.flags.bits());
        dst.put(self.fields);
        self.args.encode(dst)?;
        Ok(())
    }

    fn decode(src: &mut Bytes) -> Result<Self, TChannelError> {
        Ok(CallFieldsEncoded::new(
            decode_bitflag(src.get_u8(), Flags::from_bits)?,
            CallRequestFields::decode(src)?.encode_bytes()?,
            CallArgs::decode(src)?,
        ))
    }
}

#[derive(Debug, new)]
pub struct CallRequestFields {
    // ttl:4
    ttl: u32,
    // tracing:25
    tracing: Tracing,
    // service~1
    service: String,
    // nh:1 (hk~1, hv~1){nh}
    headers: HashMap<String, String>,
}

impl Codec for CallRequestFields {
    fn encode(self, dst: &mut BytesMut) -> Result<(), TChannelError> {
        dst.put_u32(self.ttl);
        self.tracing.encode(dst)?;
        encode_string(self.service, dst)?;
        encode_headers(self.headers, dst)?;
        Ok(())
    }

    fn decode(src: &mut Bytes) -> Result<Self, TChannelError> {
        Ok(CallRequestFields::new(
            src.get_u32(),
            Tracing::decode(src)?,
            decode_string(src)?,
            decode_headers(src)?,
        ))
    }
}

#[derive(Debug, new)]
pub struct CallRequest {
    // flags:1
    flags: Flags,
    // ttl, tracing, service name, headers
    fields: CallRequestFields,
    // checksum type, checksum, args
    args: CallArgs,
}

impl Codec for CallRequest {
    fn encode(self, dst: &mut BytesMut) -> Result<(), TChannelError> {
        dst.put_u8(self.flags.bits());
        self.fields.encode(dst);
        self.args.encode(dst)?;
        Ok(())
    }

    fn decode(src: &mut Bytes) -> Result<Self, TChannelError> {
        Ok(CallRequest::new(
            decode_bitflag(src.get_u8(), Flags::from_bits)?,
            CallRequestFields::decode(src)?,
            CallArgs::decode(src)?,
        ))
    }
}

#[derive(Debug, new)]
pub struct CallResponseFields {
    // code:1
    code: ResponseCode,
    // tracing:25
    tracing: Tracing,
    // nh:1 (hk~1, hv~1){nh}
    headers: HashMap<String, String>,
}

impl Codec for CallResponseFields {
    fn encode(self, dst: &mut BytesMut) -> Result<(), TChannelError> {
        dst.put_u8(self.code as u8);
        self.tracing.encode(dst)?;
        encode_headers(self.headers, dst)?;
        Ok(())
    }

    fn decode(src: &mut Bytes) -> Result<Self, TChannelError> {
        Ok(CallResponseFields::new(
            decode_bitflag(src.get_u8(), ResponseCode::from_u8)?,
            Tracing::decode(src)?,
            decode_headers(src)?,
        ))
    }
}

#[derive(Debug, new)]
pub struct CallResponse {
    // flags:1
    flags: Flags,
    // code, tracing, headers
    fields: CallResponseFields,
    // checksum type, checksum, args
    args: CallArgs,
}

impl Codec for CallResponse {
    fn encode(self, dst: &mut BytesMut) -> Result<(), TChannelError> {
        dst.put_u8(self.flags.bits());
        self.fields.encode(dst)?;
        self.args.encode(dst)?;
        Ok(())
    }

    fn decode(src: &mut Bytes) -> Result<Self, TChannelError> {
        Ok(CallResponse::new(
            decode_bitflag(src.get_u8(), Flags::from_bits)?,
            CallResponseFields::decode(src)?,
            CallArgs::decode(src)?,
        ))
    }
}

#[derive(Debug, new)]
pub struct CallContinue {
    // flags:1
    flags: Flags,
    // common fields
    args: CallArgs,
}

impl Codec for CallContinue {
    fn encode(self, dst: &mut BytesMut) -> Result<(), TChannelError> {
        dst.put_u8(self.flags.bits());
        self.args.encode(dst)?;
        Ok(())
    }

    fn decode(src: &mut Bytes) -> Result<Self, TChannelError> {
        Ok(CallContinue::new(
            decode_bitflag(src.get_u8(), Flags::from_bits)?,
            CallArgs::decode(src)?,
        ))
    }
}

#[derive(Debug, new)]
pub struct Cancel {
    // ttl:4
    ttl: u32,
    // tracing:25
    tracing: Tracing,
    why: String,
}

impl Codec for Cancel {
    fn encode(self, dst: &mut BytesMut) -> Result<(), TChannelError> {
        dst.put_u32(self.ttl);
        self.tracing.encode(dst)?;
        encode_string(self.why, dst)?;
        Ok(())
    }

    fn decode(src: &mut Bytes) -> Result<Self, TChannelError> {
        Ok(Cancel::new(
            src.get_u32(),
            Tracing::decode(src)?,
            decode_string(src)?,
        ))
    }
}

#[derive(Debug, new)]
pub struct Claim {
    // ttl:4
    ttl: u32,
    // tracing:25
    tracing: Tracing,
}

impl Codec for Claim {
    fn encode(self, dst: &mut BytesMut) -> Result<(), TChannelError> {
        dst.put_u32(self.ttl);
        self.tracing.encode(dst)?;
        Ok(())
    }

    fn decode(src: &mut Bytes) -> Result<Self, TChannelError> {
        Ok(Claim::new(src.get_u32(), Tracing::decode(src)?))
    }
}

// pub struct PingRequest {} // no body

// pub struct PingResponse {} // no body

#[derive(Debug, new)]
pub struct ErrorMsg {
    code: ErrorCode,
    tracing: Tracing,
    message: String,
}

impl Codec for ErrorMsg {
    fn encode(self, dst: &mut BytesMut) -> Result<(), TChannelError> {
        dst.put_u8(self.code as u8);
        self.tracing.encode(dst)?;
        encode_string(self.message, dst)?;
        Ok(())
    }

    fn decode(src: &mut Bytes) -> Result<Self, TChannelError> {
        Ok(ErrorMsg::new(
            decode_bitflag(src.get_u8(), ErrorCode::from_u8)?,
            Tracing::decode(src)?,
            decode_string(src)?,
        ))
    }
}

fn encode_headers(
    headers: HashMap<String, String>,
    dst: &mut BytesMut,
) -> Result<(), TChannelError> {
    dst.put_u16(headers.len() as u16);
    for (headerKey, headerValue) in headers {
        encode_string(headerKey, dst)?;
        encode_string(headerValue, dst)?;
    }
    Ok(())
}

fn decode_headers(src: &mut Bytes) -> Result<HashMap<String, String>, FromUtf8Error> {
    let len = src.get_u16();
    let mut headers = HashMap::new();
    for _ in 0..len {
        let key = decode_string(src)?;
        let val = decode_string(src)?;
        headers.insert(key, val);
    }
    Ok(headers)
}

fn encode_string(value: String, dst: &mut BytesMut) -> Result<(), TChannelError> {
    dst.put_u16(value.len() as u16);
    dst.write_str(value.as_str())?;
    Ok(())
}

fn decode_string(src: &mut Bytes) -> Result<String, FromUtf8Error> {
    let len = src.get_u16();
    let bytes = src.copy_to_bytes(len as usize);
    String::from_utf8(bytes.chunk().to_vec())
}

fn encode_checksum(
    checksum_type: ChecksumType,
    value: Option<u32>,
    dst: &mut BytesMut,
) -> Result<(), TChannelError> {
    dst.put_u8(checksum_type as u8);
    if checksum_type != ChecksumType::None {
        dst.put_u32(value.ok_or(TChannelError::FrameCodecError(
            "Missing checksum value.".to_owned(),
        ))?)
    }
    Ok(())
}

fn decode_checksum(src: &mut Bytes) -> Result<(ChecksumType, Option<u32>), TChannelError> {
    let checksum_type = decode_bitflag(src.get_u8(), ChecksumType::from_u8)?;
    match checksum_type {
        ChecksumType::None => Ok((ChecksumType::None, None)),
        checksum => Ok((checksum, Some(src.get_u32()))),
    }
}

fn decode_bitflag<T, F: Fn(u8) -> Option<T>>(byte: u8, decoder: F) -> Result<T, TChannelError> {
    decoder(byte).ok_or_else(|| TChannelError::FrameCodecError(format!("Unknown flag: {}", byte)))
}

fn encode_args(args: Vec<Option<Bytes>>, dst: &mut BytesMut) -> Result<(), TChannelError> {
    let args_len = args.len();
    if args_len == 0 || args_len > MAX_FRAME_ARGS {
        return Err(TChannelError::FrameCodecError(format!(
            "Wrong number of frame args {}",
            args_len
        )));
    }
    for arg in args {
        match arg {
            None => dst.put_u16(0),
            Some(arg) => {
                let len = arg.len();
                if (dst.remaining() < len + 2) {
                    return Err(TChannelError::FrameCodecError(
                        "Not enough capacity to encode arg".to_owned(),
                    ));
                }
                dst.put_u16(len as u16);
                dst.put_slice(arg.as_ref()); // take len bytes from above
            }
        }
    }
    Ok(())
}

fn decode_args(src: &mut Bytes) -> Result<Vec<Option<Bytes>>, TChannelError> {
    if src.remaining() == 0 {
        return Err(TChannelError::FrameCodecError(
            "Frame missing args".to_owned(),
        ));
    }
    let mut args = Vec::new();
    while !src.is_empty() && args.len() < MAX_FRAME_ARGS {
        args.push(decode_arg(src)?);
    }
    if !src.is_empty() {
        return Err(TChannelError::FrameCodecError(
            "Incorrect frame length".to_owned(),
        ));
    }
    Ok(args)
}

fn decode_arg(src: &mut Bytes) -> Result<Option<Bytes>, TChannelError> {
    match src.remaining() {
        0 | 1 => Err(TChannelError::FrameCodecError(
            "Cannot read arg length".to_owned(),
        )),
        remaining => match src.get_u16() {
            0 => Ok(None),
            len if len > (remaining as u16 - 2) => Err(TChannelError::FrameCodecError(
                "Wrong arg length".to_owned(),
            )),
            len => Ok(Some(src.split_to(len as usize))),
        },
    }
}
