pub mod headers;
pub mod payloads;

use crate::errors::CodecError;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use futures::Stream;
use num_traits::FromPrimitive;
use std::io::Cursor;
use std::pin::Pin;
use tokio_util::codec::{Decoder, Encoder};

pub const FRAME_HEADER_LENGTH: u16 = 16;
pub const FRAME_MAX_LENGTH: u16 = u16::MAX - 1;
pub const ZERO: u8 = 0;

#[derive(Copy, Clone, Debug, FromPrimitive, PartialEq)]
pub enum Type {
    // First message on every connection must be init
    InitRequest = 0x1,

    // Remote response to init req
    InitResponse = 0x2,

    // RPC method request
    CallRequest = 0x3,

    // RPC method response
    CallResponse = 0x4,

    // RPC request continuation fragment
    CallRequestContinue = 0x13,

    // RPC response continuation fragment
    CallResponseContinue = 0x14,

    // CancelFrame an outstanding call req / forward req (no body)
    Cancel = 0xc0,

    // ClaimFrame / cancel a redundant request
    Claim = 0xc1,

    // Protocol level ping req (no body)
    PingRequest = 0xd0,

    // PingFrame res (no body)
    PingResponse = 0xd1,

    // Protocol level error.
    Error = 0xff,
}

pub type TFrameStream = Pin<Box<dyn Stream<Item = TFrame> + Send>>;

#[derive(Debug, Getters, MutGetters, new)]
pub struct TFrame {
    #[get = "pub"]
    pub frame_type: Type,
    #[get_mut = "pub"]
    #[get = "pub"]
    payload: Bytes,
}

impl TFrame {
    pub fn size(&self) -> usize {
        self.payload.len()
    }
}

#[derive(Debug, Getters, MutGetters, new)]
pub struct TFrameId {
    #[get = "pub"]
    id: u32,
    #[get = "pub"]
    #[get_mut = "pub"]
    pub frame: TFrame,
}

#[derive(Default, Debug)]
pub struct TFrameIdCodec {}

impl Encoder<TFrameId> for TFrameIdCodec {
    type Error = crate::errors::CodecError;

    fn encode(&mut self, item: TFrameId, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let frame = item.frame();
        let len = frame.size() as u16 + FRAME_HEADER_LENGTH;
        trace!("Encoding TFrame (id {}, len {})", item.id(), frame.size());
        dst.reserve(len as usize);
        dst.put_u16(len);
        dst.put_u8(*frame.frame_type() as u8);
        dst.put_u8(ZERO); // zero
        dst.put_u32(*item.id());
        for _ in 0..8 {
            dst.put_u8(ZERO)
        }
        dst.put_slice(frame.payload());
        Ok(())
    }
}

impl Decoder for TFrameIdCodec {
    type Item = TFrameId;
    type Error = crate::errors::CodecError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if Self::is_buffering(src) {
            return Ok(None);
        }
        let size = src.get_u16();
        if size < FRAME_HEADER_LENGTH {
            return Err(CodecError::Error("Frame too short".to_owned()));
        }
        let frame_type_bytes = src.get_u8();
        let frame_type = match FromPrimitive::from_u8(frame_type_bytes) {
            Some(frame_type) => frame_type,
            None => {
                return Err(CodecError::Error(format!(
                    "Unknown frame type {}",
                    frame_type_bytes
                )))
            }
        };
        src.advance(1); // skip
        let id = src.get_u32();
        src.advance(8);
        let payload = src.split_to((size - FRAME_HEADER_LENGTH) as usize).freeze();
        let frame = TFrame {
            frame_type,
            payload,
        };
        Ok(Some(TFrameId { id, frame }))
    }
}

impl TFrameIdCodec {
    fn is_buffering(src: &mut BytesMut) -> bool {
        if src.len() < 2 {
            trace!("Cannot read length.");
            src.reserve(2 - src.len()); // Minimal required to read frame length
            return true;
        }
        let mut peeker = Cursor::new(&src[..2]);
        let size = peeker.get_u16() as usize;
        if size > src.len() {
            trace!("Buffering frame.");
            src.reserve(size + 2); // Extra 2 bytes will allow to read next frame length
            return true;
        }
        false
    }
}
