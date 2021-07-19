use crate::channel::frames::{TFrame, Type};
use crate::TChannelError;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use num_traits::FromPrimitive;
use std::collections::{HashMap, VecDeque};
use std::convert::{TryFrom, TryInto};
use std::fmt::Write;
use std::string::FromUtf8Error;

pub const PROTOCOL_VERSION: u16 = 2;
pub const TRACING_HEADER_LENGTH: u8 = 25;
pub const MAX_FRAME_ARGS: usize = 3;
/// Length of arg length frame field (2 bytes, u16)
pub const ARG_LEN_LEN: usize = 2;

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
#[derive(Debug, Getters, MutGetters, new)]
pub struct CallArgs {
    #[get = "pub"]
    // csumtype:1
    checksum_type: ChecksumType,
    #[get = "pub"]
    // (csum:4){0,1}
    checksum: Option<u32>,
    #[get_mut = "pub"]
    #[get = "pub"]
    /// arg1~2 arg2~2 arg3~2
    pub args: VecDeque<Option<Bytes>>, //TODO consider using references
}

impl Codec for CallArgs {
    fn encode(mut self, dst: &mut BytesMut) -> Result<(), TChannelError> {
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
        encode_small_string(self.service, dst)?;
        encode_small_headers(self.headers, dst)?;
        Ok(())
    }

    fn decode(src: &mut Bytes) -> Result<Self, TChannelError> {
        Ok(CallRequestFields::new(
            src.get_u32(),
            Tracing::decode(src)?,
            decode_small_string(src)?,
            decode_small_headers(src)?,
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

#[derive(Debug, Getters, new)]
pub struct CallResponseFields {
    #[get = "pub"]
    /// code:1
    pub code: ResponseCode,
    #[get = "pub"]
    /// tracing:25
    pub tracing: Tracing,
    #[get = "pub"]
    /// nh:1 (hk~1, hv~1){nh}
    pub headers: HashMap<String, String>,
}

impl Codec for CallResponseFields {
    fn encode(self, dst: &mut BytesMut) -> Result<(), TChannelError> {
        dst.put_u8(self.code as u8);
        self.tracing.encode(dst)?;
        encode_small_headers(self.headers, dst)?;
        Ok(())
    }

    fn decode(src: &mut Bytes) -> Result<Self, TChannelError> {
        Ok(CallResponseFields::new(
            decode_bitflag(src.get_u8(), ResponseCode::from_u8)?,
            Tracing::decode(src)?,
            decode_small_headers(src)?,
        ))
    }
}

#[derive(Debug, Getters, MutGetters, new)]
pub struct CallResponse {
    /// flags:1
    #[get = "pub"]
    pub flags: Flags,
    #[get = "pub"]
    /// code, tracing, headers
    pub fields: CallResponseFields,
    #[get = "pub"]
    #[get_mut = "pub"]
    /// checksum type, checksum, args
    pub args: CallArgs,
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

#[derive(Debug, Getters, MutGetters, new)]
pub struct CallContinue {
    #[get = "pub"]
    // flags:1
    flags: Flags,
    #[get = "pub"]
    #[get_mut = "pub"]
    // common fields
    pub args: CallArgs,
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
    encode_header_fields::<u16>(headers, dst, &BytesMut::put_u16, &encode_string)
}

fn encode_small_headers(
    headers: HashMap<String, String>,
    dst: &mut BytesMut,
) -> Result<(), TChannelError> {
    encode_header_fields::<u8>(headers, dst, &BytesMut::put_u8, &encode_small_string)
}

fn encode_header_fields<T: TryFrom<usize>>(
    headers: HashMap<String, String>,
    dst: &mut BytesMut,
    encode_len_fn: &dyn Fn(&mut BytesMut, T) -> (),
    encode_string: &dyn Fn(String, &mut BytesMut) -> Result<(), TChannelError>,
) -> Result<(), TChannelError> {
    encode_len(dst, headers.len(), encode_len_fn)?;
    for (headerKey, headerValue) in headers {
        encode_string(headerKey, dst)?;
        encode_string(headerValue, dst)?;
    }
    Ok(())
}

fn decode_headers(src: &mut Bytes) -> Result<HashMap<String, String>, TChannelError> {
    decode_headers_field(src, &Bytes::get_u16, &decode_string)
}

fn decode_small_headers(src: &mut Bytes) -> Result<HashMap<String, String>, TChannelError> {
    decode_headers_field(src, &Bytes::get_u8, &decode_small_string)
}

fn decode_headers_field<T: TryInto<usize>>(
    src: &mut Bytes,
    decode_len_fn: &dyn Fn(&mut Bytes) -> T,
    decode_string_fn: &dyn Fn(&mut Bytes) -> Result<String, TChannelError>,
) -> Result<HashMap<String, String>, TChannelError> {
    let len = decode_len(src, decode_len_fn)?;
    let mut headers = HashMap::new();
    for _ in 0..len {
        let key = decode_string_fn(src)?;
        let val = decode_string_fn(src)?;
        headers.insert(key, val);
    }
    Ok(headers)
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

fn encode_args(args: VecDeque<Option<Bytes>>, dst: &mut BytesMut) -> Result<(), TChannelError> {
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
                if (dst.remaining() < len + ARG_LEN_LEN) {
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

fn decode_args(src: &mut Bytes) -> Result<VecDeque<Option<Bytes>>, TChannelError> {
    if src.remaining() == 0 {
        return Err(TChannelError::FrameCodecError(
            "Frame missing args".to_owned(),
        ));
    }
    let mut args = VecDeque::new();
    while !src.is_empty() && args.len() < MAX_FRAME_ARGS {
        args.push_back(decode_arg(src)?);
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
            len if len > (remaining as u16 - 2) => Err(TChannelError::FrameCodecError(format!(
                "Wrong arg length: {}",
                len
            ))),
            len => Ok(Some(src.split_to(len as usize))),
        },
    }
}

fn encode_string(value: String, dst: &mut BytesMut) -> Result<(), TChannelError> {
    encode_string_field(value, dst, &BytesMut::put_u16)
}

fn encode_small_string(value: String, dst: &mut BytesMut) -> Result<(), TChannelError> {
    encode_string_field(value, dst, &BytesMut::put_u8)
}

fn encode_string_field<T: TryFrom<usize>>(
    value: String,
    dst: &mut BytesMut,
    encode_len_fn: &dyn Fn(&mut BytesMut, T) -> (),
) -> Result<(), TChannelError> {
    encode_len(dst, value.len(), encode_len_fn)?;
    dst.write_str(value.as_str())?;
    Ok(())
}

fn decode_string(src: &mut Bytes) -> Result<String, TChannelError> {
    decode_string_field(src, &Bytes::get_u16)
}
fn decode_small_string(src: &mut Bytes) -> Result<String, TChannelError> {
    decode_string_field(src, &Bytes::get_u8)
}

fn decode_string_field<T: Into<usize>>(
    src: &mut Bytes,
    get_len: &dyn Fn(&mut Bytes) -> T,
) -> Result<String, TChannelError> {
    let len = get_len(src);
    let bytes = src.copy_to_bytes(len.into());
    Ok(String::from_utf8(bytes.chunk().to_vec())?)
}

fn encode_len<T: TryFrom<usize>>(
    dst: &mut BytesMut,
    value: usize,
    encode_len_fn: &dyn Fn(&mut BytesMut, T) -> (),
) -> Result<(), TChannelError> {
    Ok(encode_len_fn(
        dst,
        value
            .try_into()
            .map_err(|_| TChannelError::Error(format!("Failed to cast '{}' len.", value)))?, //TODO impl From for TChannelError
    ))
}

fn decode_len<T: TryInto<usize>>(
    src: &mut Bytes,
    decode_len_fn: &dyn Fn(&mut Bytes) -> T,
) -> Result<usize, TChannelError> {
    Ok(
        decode_len_fn(src)
            .try_into()
            .map_err(|_| TChannelError::Error(format!("Failed to cast len to usize.")))?, //TODO impl From for TChannelError
    )
}
