use crate::config::Config;
use crate::connection::pool::ConnectionPools;
use crate::connection::ConnectionResult;
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

/// TChannel protocol. Keeps started server handle and created [`SubChannel`s](crate::SubChannel).
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
        let connection_pools = ConnectionPools::new(config.clone());
        let subchannels = Arc::new(RwLock::new(HashMap::new()));
        Ok(TChannel {
            config,
            connection_pools: Arc::new(connection_pools),
            subchannels,
            server_handle: Mutex::new(None),
        })
    }

    /// Starts server task.
    pub fn start_server(&self) -> TResult<()> {
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

    /// Stops server task.
    pub fn shutdown_server(&self) -> TResult<()> {
        let mut handle_lock = self
            .server_handle
            .lock()
            .map_err(|err| TChannelError::Error(err.to_string()))?; //TODO new error type?
        if let Some(handle) = handle_lock.deref() {
            info!("Stopping server");
            handle.abort(); //TODO timeout?
            *handle_lock = None;
        } else {
            debug!("Server already stopped. Nothing to do.");
        }
        Ok(())
    }

    /// Gets `self::channel::SubChannel` for given `service_name`. If not found a new instance is created.
    pub async fn subchannel(&self, service_name: impl AsRef<str>) -> TResult<Arc<SubChannel>> {
        if let Some(subchannel) = self.subchannels.read().await.get(service_name.as_ref()) {
            return Ok(subchannel.clone());
        }
        self.create_subchannel(service_name).await
    }

    async fn create_subchannel(&self, service_name: impl AsRef<str>) -> TResult<Arc<SubChannel>> {
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
    // Mainly to log shutting down.
    fn drop(&mut self) {
        if let Err(err) = self.shutdown_server() {
            error!("Failed to shutdown server on drop: {}", err.to_string());
        }
    }
}
