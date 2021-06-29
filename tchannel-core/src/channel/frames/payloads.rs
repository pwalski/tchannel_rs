use crate::TChannelError;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use num_traits::FromPrimitive;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt::Write;
use std::string::FromUtf8Error;

pub const PROTOCOL_VERSION: u16 = 2;
pub const TRACING_HEADER_LENGTH: u8 = 25;

pub trait Codec: Sized {
    fn encode(self, dst: &mut BytesMut) -> Result<(), TChannelError>;
    fn decode(src: &mut Bytes) -> Result<Self, TChannelError>;
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

#[derive(Debug, new)]
pub struct CallCommonFields {
    // nh:1 (hk~1, hv~1){nh}
    headers: HashMap<String, String>,
    // csumtype:1
    checksum_type: ChecksumType,
    // (csum:4){0,1}
    checksum: Option<u32>,
    // arg1~2 arg2~2 arg3~2
    arg1: Option<Bytes>,
    arg2: Option<Bytes>,
    arg3: Option<Bytes>,
}

impl Codec for CallCommonFields {
    fn encode(mut self, dst: &mut BytesMut) -> Result<(), TChannelError> {
        encode_headers(self.headers, dst)?;
        dst.put_u8(self.checksum_type as u8);
        encode_checksum(self.checksum_type, self.checksum, dst)?;
        encode_arg(&self.arg1, dst)?;
        encode_arg(&self.arg2, dst)?;
        encode_arg(&self.arg3, dst)?;
        Ok(())
    }

    fn decode(src: &mut Bytes) -> Result<Self, TChannelError> {
        let headers = decode_headers(src)?;
        let (checksum_type, checksum) = decode_checksum(src)?;
        Ok(CallCommonFields::new(
            headers,
            checksum_type,
            checksum,
            decode_arg(src)?, //arg1
            decode_arg(src)?, //arg2
            decode_arg(src)?, //arg3
        ))
    }
}

#[derive(Debug, new)]
struct CallRequest {
    // flags:1
    flags: Flags,
    // ttl:4
    ttl: u32,
    // tracing:25
    tracing: Tracing,
    // service~1
    service: String,
    // common fields
    fields: CallCommonFields,
}

impl Codec for CallRequest {
    fn encode(self, dst: &mut BytesMut) -> Result<(), TChannelError> {
        dst.put_u8(self.flags.bits());
        dst.put_u32(self.ttl);
        self.tracing.encode(dst)?;
        encode_string(self.service, dst)?;
        self.fields.encode(dst)?;
        Ok(())
    }

    fn decode(src: &mut Bytes) -> Result<Self, TChannelError> {
        Ok(CallRequest::new(
            decode_bitflag(src.get_u8(), Flags::from_bits)?,
            src.get_u32(),
            Tracing::decode(src)?,
            decode_string(src)?,
            CallCommonFields::decode(src)?,
        ))
    }
}

#[derive(Debug, new)]
struct CallResponse {
    // flags:1
    flags: Flags,
    // code:1
    code: ResponseCode,
    // tracing:25
    tracing: Tracing,
    // common fields
    fields: CallCommonFields,
}

impl Codec for CallResponse {
    fn encode(self, dst: &mut BytesMut) -> Result<(), TChannelError> {
        dst.put_u8(self.flags.bits());
        dst.put_u8(self.code as u8);
        self.tracing.encode(dst)?;
        self.fields.encode(dst)?;
        Ok(())
    }

    fn decode(src: &mut Bytes) -> Result<Self, TChannelError> {
        Ok(CallResponse::new(
            decode_bitflag(src.get_u8(), Flags::from_bits)?,
            decode_bitflag(src.get_u8(), ResponseCode::from_u8)?,
            Tracing::decode(src)?,
            CallCommonFields::decode(src)?,
        ))
    }
}

#[derive(Debug, new)]
pub struct CallContinue {
    // flags:1
    flags: Flags,
    // common fields
    fields: CallCommonFields,
}

impl Codec for CallContinue {
    fn encode(self, dst: &mut BytesMut) -> Result<(), TChannelError> {
        dst.put_u8(self.flags.bits());
        self.fields.encode(dst)?;
        Ok(())
    }

    fn decode(src: &mut Bytes) -> Result<Self, TChannelError> {
        Ok(CallContinue::new(
            decode_bitflag(src.get_u8(), Flags::from_bits)?,
            CallCommonFields::decode(src)?,
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
            "Missing checksum value.".to_string(),
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

fn encode_arg(src: &Option<Bytes>, dst: &mut BytesMut) -> Result<(), TChannelError> {
    match src {
        None => Ok(()),
        Some(arg) => {
            if (dst.remaining_mut() < arg.len() + 2) {
                return Err(TChannelError::FrameCodecError(
                    "Not enough capacity to save arg".to_string(),
                ));
            }
            dst.put_u16(arg.len() as u16);
            dst.put_slice(arg.as_ref()); // take len bytes from above
            Ok(())
        }
    }
}

fn decode_arg(src: &mut Bytes) -> Result<Option<Bytes>, TChannelError> {
    match src.remaining() {
        0 => Ok(None),
        1 => Err(TChannelError::FrameCodecError(
            "Cannot read call arg length".to_string(),
        )),
        remaining => match src.get_u16() {
            0 => Ok(None),
            len if len > (remaining as u16 - 2) => Err(TChannelError::FrameCodecError(
                "Wrong call arg length".to_string(),
            )),
            len => Ok(Some(src.split_to(len as usize))),
        },
    }
}
