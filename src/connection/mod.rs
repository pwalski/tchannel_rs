pub mod pool;

use crate::errors::ConnectionError;
use crate::frames::{TFrame, TFrameId, TFrameIdCodec};
use core::time::Duration;
use futures::prelude::*;
use futures::{self, SinkExt};
use futures::{future, StreamExt};
use log::{debug, error};
use std::collections::HashMap;
use std::fmt::Debug;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::Mutex;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::codec::{FramedRead, FramedWrite};

#[derive(Debug, Builder, Clone)]
pub struct ConnectionOptions {
    max_connections: u32,
    lifetime: Option<Duration>,
    test_connection: bool,
    frame_buffer_size: usize,
}

impl Default for ConnectionOptions {
    fn default() -> Self {
        ConnectionOptions {
            max_connections: 1,
            lifetime: Some(Duration::from_secs(60)),
            test_connection: false,
            frame_buffer_size: 100,
        }
    }
}

#[derive(Default, Debug)]
pub struct PendingIds {
    channels: RwLock<HashMap<u32, Sender<TFrameId>>>,
}

impl PendingIds {
    pub async fn add(&self, id: u32, sender: Sender<TFrameId>) {
        let mut channels = self.channels.write().await;
        channels.insert(id, sender);
    }

    pub async fn respond(&self, response: TFrameId) -> Result<(), ConnectionError> {
        let id = *response.id();
        let channels = self.channels.read().await;
        if let Some(sender) = channels.get(&id) {
            return Ok(sender.send(response).await?);
        }
        Err(ConnectionError::Error(format!("Id {} not found", id)))
    }

    pub async fn remove(&self, id: &u32) -> Option<Sender<TFrameId>> {
        let mut channels = self.channels.write().await;
        channels.remove(id)
    }
}

#[derive(Debug, Builder)]
#[builder(pattern = "owned")]
pub struct Connection {
    #[builder(setter(skip))]
    next_message_id: AtomicU32,
    #[builder(setter(skip))]
    pending_ids: Arc<PendingIds>,
    sender: tokio::sync::mpsc::Sender<TFrameId>,
}

impl Connection {
    pub async fn connect(
        addr: SocketAddr,
        buffer_size: usize,
    ) -> Result<Connection, ConnectionError> {
        debug!("Connecting to {}", addr);
        let tcp_stream = TcpStream::connect(addr).await?;
        let (read, write) = tcp_stream.into_split();
        let framed_read = FramedRead::new(read, TFrameIdCodec {});
        let framed_write = FramedWrite::new(write, TFrameIdCodec {});
        let (sender, receiver) = mpsc::channel::<TFrameId>(buffer_size);
        let connection = ConnectionBuilder::default().sender(sender).build()?;
        FrameReceiver::spawn(framed_read, connection.pending_ids.clone());
        FrameSender::spawn(framed_write, receiver, buffer_size);
        Ok(connection)
    }

    /// Prepares frame I/O with new message id. Then sends `frame` and awaits for response.
    pub async fn send_one(&self, frame: TFrame) -> Result<TFrameId, ConnectionError> {
        let (frame_output, mut frame_receiver) = self.new_frame_io().await;
        frame_output.send(frame).await?;
        let response = frame_receiver.recv().await;
        frame_output.close().await;
        response.ok_or_else(|| ConnectionError::Error("Received no response".to_owned()))
    }

    /// Prepares frame I/O with new message id.
    /// Then returns both input and output which allows to send multiple frames with same message id.
    pub async fn new_frame_io(&self) -> (FrameOutput, FrameInput) {
        let message_id = self.next_message_id();
        let (sender, receiver) = mpsc::channel::<TFrameId>(10); //TODO connfigure
        self.pending_ids.add(message_id, sender).await;
        let frame_output =
            FrameOutput::new(message_id, self.sender.clone(), self.pending_ids.clone());
        (frame_output, receiver)
    }

    fn next_message_id(&self) -> u32 {
        self.next_message_id.fetch_add(1, Ordering::Relaxed)
    }
}

pub type FrameInput = Receiver<TFrameId>;

#[derive(Getters, new)]
pub struct FrameOutput {
    message_id: u32,
    sender: Sender<TFrameId>,
    pending_ids: Arc<PendingIds>,
}

impl FrameOutput {
    pub async fn send(&self, frame: TFrame) -> Result<(), ConnectionError> {
        let frame = TFrameId::new(self.message_id, frame);
        debug!("Passing frame {:?} to sender.", frame);
        Ok(self.sender.send(frame).await?)
    }

    //TODO figure out how to automatically close it? impl Sink? do it on Deref?
    pub async fn close(&self) {
        self.pending_ids.remove(&self.message_id).await;
    }
}

struct FrameSender {
    framed_write: Arc<Mutex<FramedWrite<OwnedWriteHalf, TFrameIdCodec>>>,
    buffer_size: usize,
}

impl FrameSender {
    pub fn spawn(
        framed_write: FramedWrite<OwnedWriteHalf, TFrameIdCodec>,
        frame_receiver: tokio::sync::mpsc::Receiver<TFrameId>,
        buffer_size: usize,
    ) -> JoinHandle<()> {
        let frame_sender = FrameSender {
            framed_write: Arc::new(Mutex::new(framed_write)),
            buffer_size,
        };
        tokio::spawn(async move {
            frame_sender
                .run(ReceiverStream::new(frame_receiver))
                .then(|_| async { debug!("FrameSender stopped") })
                .await
        })
    }

    async fn run(&self, receiver_stream: ReceiverStream<TFrameId>) {
        debug!("Starting FrameSender");
        receiver_stream
            .ready_chunks(self.buffer_size)
            .for_each(|frames| self.send_frames(frames))
            .await
    }

    async fn send_frames(&self, frames: Vec<TFrameId>) {
        let mut framed_write = self.framed_write.lock().await;
        for frame in frames {
            debug!("Writing frame (id: {})", frame.id());
            if let Err(err) = framed_write.send(frame).await {
                error!("Failed to write frame: {}", err);
            }
        }
        debug!("Flushing frames");
        if let Err(err) = framed_write.flush().await {
            error!("Failed to flush frames: {}", err);
        }
    }
}

struct FrameReceiver {
    pending_ids: Arc<PendingIds>,
}

impl FrameReceiver {
    pub fn spawn(
        framed_read: FramedRead<OwnedReadHalf, TFrameIdCodec>,
        pending_ids: Arc<PendingIds>,
    ) -> JoinHandle<()> {
        let frame_receiver = FrameReceiver { pending_ids };
        tokio::spawn(async move {
            frame_receiver
                .run(framed_read)
                .then(|_| async { debug!("FrameReceiver stopped") })
                .await
        })
    }

    async fn run(&self, frame_input: FramedRead<OwnedReadHalf, TFrameIdCodec>) {
        debug!("Starting FrameReceiver");
        frame_input
            .filter_map(|frame_res| match frame_res {
                Ok(frame) => future::ready(Some(frame)),
                Err(err) => {
                    error!("Failed to serialize frame: {:?}", err);
                    future::ready(None)
                }
            })
            .map(|frame| {
                debug!("Received frame id: {}", frame.id());
                self.pending_ids.respond(frame)
            })
            .for_each(|res| async {
                if let Some(err) = res.await.err() {
                    error!("Failed to send frame: {:?}", err);
                }
            })
            .await
    }
}
