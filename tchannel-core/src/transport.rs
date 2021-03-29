use crate::frame::{IFrame, TFrame, Type};
use crate::frame::{FRAME_HEADER_LENGTH, ZERO};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::io::Error;
use std::io::Result;
use std::iter::Map;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio_util::codec::{Decoder, Encoder, Framed};

use std::convert::TryInto;
use std::fmt;
use std::io::Cursor;
use std::num::TryFromIntError;
use std::string::FromUtf8Error;

use std::collections::HashMap;

pub struct Connection {
    transport: Framed<TcpStream, TFrameCodec>,
}

// pub fn connect(host: String) -> &Connection {
//     unimplemented!()
// }

#[derive(Default, Debug)]
pub struct TFrameCodec;

impl TFrameCodec {}

impl Decoder for TFrameCodec {
    type Item = TFrame;
    type Error = std::io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>> {
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
    type Error = std::io::Error;

    fn encode(&mut self, item: TFrame, dst: &mut BytesMut) -> Result<()> {
        let len = item.getSize() as u16 + FRAME_HEADER_LENGTH;
        dst.reserve(len as usize);
        dst.put_u16(len);
        dst.put_u8(item.getType() as u8);
        dst.put_u8(ZERO); // zero
        dst.put_u32(item.getId());
        for _ in 0..8 {
            dst.put_u8(ZERO)
        }
        dst.put_slice(item.getPayload());
        Ok(())
    }
}
