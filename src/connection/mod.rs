use crate::errors::ConnectionError;
use crate::frames::payloads::CodecResult;
use crate::frames::{TFrame, TFrameId, TFrameIdCodec};
use core::time::Duration;
use futures::prelude::*;
use futures::{self, SinkExt};
use futures::{future, StreamExt};
use log::{debug, error};
use std::collections::HashMap;
use std::fmt::Debug;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
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

pub mod pool;

pub type ConnectionResult<T> = Result<T, ConnectionError>;

#[derive(Debug, Builder)]
pub struct Config {
    pub(crate) max_connections: u32,
    pub(crate) lifetime: Option<Duration>,
    pub(crate) test_connection: bool,
    pub(crate) frame_buffer_size: usize,
    pub(crate) server_address: SocketAddr,
    pub(crate) server_tasks: usize,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            max_connections: 1,
            lifetime: Some(Duration::from_secs(60)),
            test_connection: false,
            frame_buffer_size: 100,
            server_address: SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8888),
            server_tasks: 4,
        }
    }
}

pub type FrameInput = Receiver<TFrameId>;

#[derive(Debug, new)]
pub struct FrameOutput {
    message_id: u32,
    sender: Sender<TFrameId>,
    frames_dispatcher: Arc<FramesDispatcher>,
}

impl FrameOutput {
    pub async fn send(&self, frame: TFrame) -> ConnectionResult<()> {
        let frame = TFrameId::new(self.message_id, frame);
        trace!("Passing frame {:?} to sender.", frame);
        Ok(self.sender.send(frame).await?)
    }

    //TODO figure out how to automatically close it? impl Sink? do it on Drop (which is not async)?
    pub async fn close(&self) {
        self.frames_dispatcher.deregister(&self.message_id).await;
    }
}

/// Pending Message Ids mapped to Senders of frames for given message Id.
#[derive(Debug, Default, new)]
pub struct FramesDispatcher {
    #[new(default)]
    senders: RwLock<HashMap<u32, Sender<TFrameId>>>,
    buffer_size: usize,
}

/// Dispatches frames to channels according to their IDs.
impl FramesDispatcher {
    /// Dispatches frame to channel.
    /// Returns `Receiver` of new channel if it got created for given frame.
    pub async fn dispatch(&self, frame: TFrameId) -> ConnectionResult<Option<FrameInput>> {
        let senders = self.senders.read().await;
        if let Some(sender) = senders.get(frame.id()) {
            debug!("Dispatching frame (id {}) to found sender.", frame.id());
            return Ok(sender.send(frame).map_ok(|_| None).await?);
        }
        std::mem::drop(senders);
        //TODO it will return error on check if it got concurrently inserted
        Ok(self.dispatch_first(frame).map_ok(Some).await?)
    }

    pub async fn deregister(&self, id: &u32) -> Option<Sender<TFrameId>> {
        let mut channels = self.senders.write().await;
        channels.remove(id)
    }

    pub async fn register(&self, id: u32, sender: Sender<TFrameId>) -> ConnectionResult<()> {
        let mut channels = self.senders.write().await;
        if channels.insert(id, sender).is_some() {
            debug!("Sender for id {} already exists", id);
            return Err(ConnectionError::Error(format!("Duplicated id: {}.", id)));
        }
        Ok(())
    }

    async fn dispatch_first(&self, frame: TFrameId) -> ConnectionResult<Receiver<TFrameId>> {
        let id = *frame.id();
        debug!("Received frame with id: {}", id);
        let mut senders = self.senders.write().await;
        if let Some(_sender) = senders.get(&id) {
            let msg = format!("Sender for id {} exists", id);
            return Err(ConnectionError::Error(msg));
        }
        let (sender, receiver) = mpsc::channel::<TFrameId>(self.buffer_size);
        sender.send(frame).await?;
        senders.insert(id, sender);
        Ok(receiver)
    }

    pub async fn dispatch_following(&self, frame: TFrameId) -> ConnectionResult<()> {
        let id = *frame.id();
        debug!("Received frame with id: {}", id);
        let senders = self.senders.read().await;
        if let Some(sender) = senders.get(&id) {
            return Ok(sender.send(frame).await?);
        }
        Err(ConnectionError::Error(format!("Id {} not found", id)))
    }
}

#[derive(Debug)]
pub struct Connection {
    next_message_id: AtomicU32,
    pending_ids: Arc<FramesDispatcher>,
    sender: Sender<TFrameId>,
    buffer_size: usize,
}

impl Connection {
    pub fn new(sender: Sender<TFrameId>, buffer_size: usize) -> Connection {
        Connection {
            next_message_id: AtomicU32::default(),
            pending_ids: Arc::new(FramesDispatcher::new(buffer_size)),
            sender,
            buffer_size,
        }
    }

    pub async fn connect(stream: TcpStream, buffer_size: usize) -> ConnectionResult<Connection> {
        let (read, write) = stream.into_split();
        let framed_read = FramedRead::new(read, TFrameIdCodec {});
        let framed_write = FramedWrite::new(write, TFrameIdCodec {});
        let (sender, receiver) = mpsc::channel::<TFrameId>(buffer_size);
        let connection = Connection::new(sender, buffer_size);
        FrameReceiver::spawn(framed_read, connection.pending_ids.clone());
        FrameSender::spawn(framed_write, receiver, buffer_size);
        Ok(connection)
    }

    /// Prepares frame I/O with new message id. Then sends `frame` and awaits for response.
    pub async fn send_one(&self, frame: TFrame) -> ConnectionResult<TFrameId> {
        let (mut frame_input, frame_output) = self.new_frames_io().await?;
        frame_output.send(frame).await?;
        let response = frame_input.recv().await;
        frame_output.close().await;
        response.ok_or_else(|| ConnectionError::Error("Received no response".to_owned()))
    }

    /// Prepares frame I/O with new message id.
    /// Then returns both input and output which allows to send multiple frames with same message id.
    pub async fn new_frames_io(&self) -> ConnectionResult<(FrameInput, FrameOutput)> {
        let message_id = self.next_message_id();
        let (sender, receiver) = mpsc::channel::<TFrameId>(self.buffer_size); //TODO should use bufer_size?
        self.pending_ids.register(message_id, sender).await?;
        let frame_output =
            FrameOutput::new(message_id, self.sender.clone(), self.pending_ids.clone());
        Ok((receiver, frame_output))
    }

    fn next_message_id(&self) -> u32 {
        self.next_message_id.fetch_add(1, Ordering::Relaxed)
    }
}

pub struct FrameSender {
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
    frame_senders: Arc<FramesDispatcher>,
}

impl FrameReceiver {
    pub fn spawn(
        framed_read: FramedRead<OwnedReadHalf, TFrameIdCodec>,
        frame_senders: Arc<FramesDispatcher>,
    ) -> JoinHandle<()> {
        let frame_receiver = FrameReceiver { frame_senders };
        tokio::spawn(async move {
            frame_receiver
                .run(framed_read)
                .then(|_| async { debug!("FrameReceiver stopped") })
                .await
        })
    }

    async fn run(&self, framed_read: FramedRead<OwnedReadHalf, TFrameIdCodec>) {
        debug!("Starting FrameReceiver");
        framed_read
            .filter_map(Self::print_if_err_and_skip)
            .map(|frame| self.frame_senders.dispatch_following(frame))
            .for_each(Self::print_if_err)
            .await
    }

    fn print_if_err_and_skip(
        frame_res: CodecResult<TFrameId>,
    ) -> impl Future<Output = Option<TFrameId>> {
        match frame_res {
            Ok(frame) => future::ready(Some(frame)),
            Err(err) => {
                error!("Frame handling failure: {:?}", err);
                future::ready(None)
            }
        }
    }

    async fn print_if_err(some_res: impl Future<Output = ConnectionResult<()>>) {
        if let Some(err) = some_res.await.err() {
            error!("Failed to send frame: {:?}", err);
        }
    }
}
