use crate::frame::{self, Frame, IFrame};

use crate::frame::{FRAME_HEADER_LENGTH,ZERO};

use bytes::{Buf, BytesMut, BufMut};
use std::io::{self, Cursor};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufWriter};
use tokio::net::TcpStream;

#[derive(Debug)]
pub struct Connection {
    socket: BufWriter<TcpStream>,
    buffer: BytesMut,
}

pub enum Direction {
    None,
    In,
    Out
}

impl Connection {
    pub fn new(socket: TcpStream) -> Connection {
        Connection {
            socket: BufWriter::new(socket),
            buffer: BytesMut::with_capacity(4 * 1024),
        }
    }

    pub async fn write_iframe(&mut self, frame: &IFrame) -> io::Result<()> {

        let mut buf = BytesMut::with_capacity(FRAME_HEADER_LENGTH as usize);
        buf.put_u16(frame.getSize() as u16 + FRAME_HEADER_LENGTH);
        buf.put_u8(frame.getType() as u8);
        buf.put_u8(ZERO); // zero
        buf.put_u32(frame.getId());
        for _ in 0..8 {
            buf.put_u8(ZERO)
        }
        let frame_payload = buf.chain(frame.getPayload().chunk());


        println!("writing payload");
        while frame_payload.has_remaining() {
            println!("writing");
            self.socket.write_all(frame_payload.chunk()).await;
        }
        println!("wrote bytes");

        self.socket.flush().await
    }

    pub async fn read_frame(&mut self) -> crate::Result<Option<Frame>> {
        loop {
            println!("Parsing frame");
            if let Some(frame) = self.parse_frame()? {
                println!("Parsed frame");
                return Ok(Some(frame));
            }
            println!("Reading");
            if 0 == self.socket.read_buf(&mut self.buffer).await? {
                if self.buffer.is_empty() {
                    return Ok(None);
                } else {
                    return Err("connection reset by peer".into());
                }
            }
        }
    }

    fn parse_frame(&mut self) -> crate::Result<Option<Frame>> {
        use frame::Error::Incomplete;
        let mut buf = Cursor::new(&self.buffer[..]);
        match Frame::check(&mut buf) {
            Ok(_) => {
                let len = buf.position() as usize;
                buf.set_position(0);
                let frame = Frame::parse(&mut buf)?;
                self.buffer.advance(len);
                Ok(Some(frame))
            }
            Err(Incomplete) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
    
    pub async fn write_frame(&mut self, frame: &Frame) -> io::Result<()> {
        match frame {
            Frame::Array(val) => {
                self.socket.write_u8(b'*').await?;
                self.write_decimal(val.len() as u64).await?;
                for entry in &**val {
                    self.write_value(entry).await?;
                }
            }
            _ => self.write_value(frame).await?,
        }
        self.socket.flush().await
    }

    async fn write_value(&mut self, frame: &Frame) -> io::Result<()> {
        match frame {
            Frame::InitReq { id, version, headers} => {
                println!("id {} version {}", id, version);

                self.socket.write_i16(*id);
            }


            Frame::Simple(val) => {
                self.socket.write_u8(b'+').await?;
                self.socket.write_all(val.as_bytes()).await?;
                self.socket.write_all(b"\r\n").await?;
            }
            Frame::Error(val) => {
                self.socket.write_u8(b'-').await?;
                self.socket.write_all(val.as_bytes()).await?;
                self.socket.write_all(b"\r\n").await?;
            }
            Frame::Integer(val) => {
                self.socket.write_u8(b':').await?;
                self.write_decimal(*val).await?;
            }
            Frame::Null => {
                self.socket.write_all(b"$-1\r\n").await?;
            }
            Frame::Bulk(val) => {
                let len = val.len();
                self.socket.write_u8(b'$').await?;
                self.write_decimal(len as u64).await?;
                self.socket.write_all(val).await?;
                self.socket.write_all(b"\r\n").await?;
            }
            Frame::Array(_val) => unreachable!(),
        }

        Ok(())
    }

    async fn write_decimal(&mut self, val: u64) -> io::Result<()> {
        use std::io::Write;
        let mut buf = [0u8; 20];
        let mut buf = Cursor::new(&mut buf[..]);
        write!(&mut buf, "{}", val)?;
        let pos = buf.position() as usize;
        self.socket.write_all(&buf.get_ref()[..pos]).await?;
        self.socket.write_all(b"\r\n").await?;

        Ok(())
    }
}
