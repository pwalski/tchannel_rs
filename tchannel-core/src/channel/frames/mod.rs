pub mod payloads;

use crate::TChannelError;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use tokio_util::codec::{Decoder, Encoder};

pub const FRAME_HEADER_LENGTH: u16 = 16;
pub const ZERO: u8 = 0;

#[derive(Copy, Clone, Debug, FromPrimitive)]
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
    Error = 0x00,
}

#[derive(Debug, Getters, Builder)]
pub struct TFrame {
    #[get = "pub"]
    frame_type: Type,
    #[get = "pub"]
    payload: Bytes,
}

impl TFrame {
    pub fn size(&self) -> usize {
        self.payload.len()
    }
}

#[derive(Debug, Getters, new)]
pub struct TFrameId {
    #[get = "pub"]
    id: u32,
    #[get = "pub"]
    frame: TFrame,
}

#[derive(Default, Debug)]
pub struct TFrameIdCodec {}

impl Encoder<TFrameId> for TFrameIdCodec {
    type Error = crate::TChannelError;

    fn encode(&mut self, item: TFrameId, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let frame = item.frame();
        let len = frame.size() as u16 + FRAME_HEADER_LENGTH;
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
    type Error = crate::TChannelError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.is_empty() {
            debug!("Empty bytes src");
            return Ok(None);
        }
        let size = src.get_u16();
        if size < FRAME_HEADER_LENGTH {
            return Err(TChannelError::FrameCodecError(
                "Frame too short".to_string(),
            ));
        }
        let frame_type = num::FromPrimitive::from_u8(src.get_u8()).unwrap();
        src.advance(1); // skip
        let id = src.get_u32();
        src.advance(8);
        let payload = src.split_to((size - FRAME_HEADER_LENGTH) as usize).freeze();
        let frame = TFrame {
            frame_type: frame_type,
            payload: payload,
        };
        Ok(Some(TFrameId { id, frame }))
    }
}
