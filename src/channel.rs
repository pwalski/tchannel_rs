use crate::connection::pool::{ConnectionPools, ConnectionPoolsBuilder};
use crate::connection::{Config, ConnectionResult};
use crate::errors::TChannelError;
use crate::server::Server;
use crate::SubChannel;
use log::debug;
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::{Arc, Mutex};
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

pub(crate) type SharedSubChannels = Arc<RwLock<HashMap<String, Arc<SubChannel>>>>;
// Mutex to be Sync, Cell to get owned type on Drop impl TChannel, Option for lazy initialization.
type ServerHandle = Mutex<Option<JoinHandle<ConnectionResult<()>>>>;

/// TChannel general result.
pub type TResult<T> = Result<T, TChannelError>;

/// TChannel protocol.
pub struct TChannel {
    config: Arc<Config>,
    connection_pools: Arc<ConnectionPools>,
    subchannels: SharedSubChannels,
    server_handle: ServerHandle,
}

impl TChannel {
    /// Initializes TChannel.
    pub fn new(config: Config) -> TResult<Self> {
        let config = Arc::new(config);
        let connection_pools = ConnectionPoolsBuilder::default()
            .config(config.clone())
            .build()?;
        let subchannels = Arc::new(RwLock::new(HashMap::new()));
        Ok(TChannel {
            config,
            connection_pools: Arc::new(connection_pools),
            subchannels,
            server_handle: Mutex::new(None),
        })
    }

    /// Starts server
    pub fn start_server(&mut self) -> TResult<()> {
        let mut handle_lock = self.server_handle.lock().unwrap();
        if handle_lock.is_none() {
            debug!("Starting server"); //TODO lie
            let handle = tokio::spawn(Server::run(self.config.clone(), self.subchannels.clone()));
            *handle_lock = Some(handle);
        } else {
            return Err(TChannelError::Error("Server already started".to_string()));
        }
        Ok(())
    }

    pub fn shutdown_server(&mut self) {
        //TODO new error type?
        let mut handle_lock = self
            .server_handle
            .lock()
            .expect("Cannot lock on server task handle.");
        //TODO dirty workaround for ability to shutdown Runtime having only &self (Drop impl).
        if let Some(handle) = handle_lock.deref() {
            info!("Stopping server");
            //TODO timeout?
            handle.abort();
            *handle_lock = None;
        } else {
            debug!("Server already stopped. Nothing to do.");
        }
    }

    pub async fn subchannel<STR: AsRef<str>>(&self, service_name: STR) -> TResult<Arc<SubChannel>> {
        if let Some(subchannel) = self.subchannels.read().await.get(service_name.as_ref()) {
            return Ok(subchannel.clone());
        }
        self.create_subchannel(service_name).await
    }

    async fn create_subchannel<STR: AsRef<str>>(
        &self,
        service_name: STR,
    ) -> TResult<Arc<SubChannel>> {
        let mut subchannels = self.subchannels.write().await;
        match subchannels.get(service_name.as_ref()) {
            Some(subchannel) => Ok(subchannel.clone()),
            None => {
                debug!("Creating subchannel {}", service_name.as_ref());
                let subchannel = Arc::new(SubChannel::new(
                    String::from(service_name.as_ref()),
                    self.connection_pools.clone(),
                ));
                subchannels.insert(String::from(service_name.as_ref()), subchannel.clone());
                Ok(subchannel)
            }
        }
    }
}

impl Drop for TChannel {
    fn drop(&mut self) {
        self.shutdown_server();
    }
}
