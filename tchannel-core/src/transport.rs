use crate::frame::{IFrame, TFrame};
use crate::frame::{FRAME_HEADER_LENGTH, ZERO};
use crate::TChannelError;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use tokio::net::TcpStream;
use tokio_util::codec::{Decoder, Encoder, Framed};

pub struct Connection {
    transport: Framed<TcpStream, TFrameCodec>,
}

// pub fn connect(host: String) -> &Connection {
//     unimplemented!()
// }

#[derive(Default, Debug)]
pub struct TFrameCodec;

impl Decoder for TFrameCodec {
    type Item = TFrame;
    type Error = TChannelError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, TChannelError> {
        let size = src.get_u16() - FRAME_HEADER_LENGTH;
        let frame_type = num::FromPrimitive::from_u8(src.get_u8()).unwrap(); // if not?
        src.advance(1); // skip
        let id = src.get_u32();
        src.advance(8);
        let payload = Bytes::from(src.split());
        let frame = TFrame {
            id: id,
            frame_type: frame_type,
            payload: payload,
        };
        Ok(Some(frame))
    }
}

impl Encoder<TFrame> for TFrameCodec {
    type Error = TChannelError;

    fn encode(&mut self, item: TFrame, dst: &mut BytesMut) -> Result<(), TChannelError> {
        let len = item.size() as u16 + FRAME_HEADER_LENGTH;
        dst.reserve(len as usize);
        dst.put_u16(len);
        dst.put_u8(item.frame_type() as u8);
        dst.put_u8(ZERO); // zero
        dst.put_u32(item.id());
        for _ in 0..8 {
            dst.put_u8(ZERO)
        }
        dst.put_slice(item.payload());
        Ok(())
    }
}
