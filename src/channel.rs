use crate::connection::pool::{ConnectionPools, ConnectionPoolsBuilder};
use crate::connection::Config;
use crate::errors::{ConnectionError, TChannelError};
use crate::server::Server;
use crate::SubChannel;
use log::debug;
use std::cell::Cell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::runtime::Runtime;
use tokio::sync::RwLock;

pub(crate) type SharedSubChannels = Arc<RwLock<HashMap<String, Arc<SubChannel>>>>;
// Mutex to be Sync, Cell to get owned type on Drop impl TChannel, Option for lazy initialization.
type OptionalRuntime = Mutex<Cell<Option<Runtime>>>;

pub struct TChannel {
    config: Arc<Config>,
    connection_pools: Arc<ConnectionPools>,
    subchannels: SharedSubChannels,
    server_runtime: OptionalRuntime,
}

impl TChannel {
    /// Initializes TChannel.
    pub fn new(config: Config) -> Result<Self, TChannelError> {
        let config = Arc::new(config);
        let connection_pools = ConnectionPoolsBuilder::default()
            .config(config.clone())
            .build()?;
        let subchannels = Arc::new(RwLock::new(HashMap::new()));
        Ok(TChannel {
            config,
            connection_pools: Arc::new(connection_pools),
            subchannels,
            server_runtime: Mutex::new(Cell::new(None)),
        })
    }

    /// Starts server
    pub fn start_server(&mut self) -> Result<(), ConnectionError> {
        let mut runtime_lock = self.server_runtime.lock().unwrap();
        if runtime_lock.get_mut().is_none() {
            debug!("Server runtime is alive"); //TODO lie
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_io()
                .max_blocking_threads(self.config.max_server_threads)
                .thread_name("tchannel_server")
                .build()?;
            runtime.spawn(Server::run(self.config.clone(), self.subchannels.clone()));
            runtime_lock.set(Some(runtime));
        }
        Ok(())
    }

    pub fn shutdown_server(&self) {
        let runtime_lock = self.server_runtime.lock().unwrap();
        //TODO dirty workaround for ability to shutdown Runtime having only &self (Drop impl).
        if let Some(runtime) = runtime_lock.replace(None) {
            debug!("Stopping server");
            runtime.shutdown_timeout(Duration::from_millis(100));
            // runtime.shutdown_background();
            debug!("Server stopped");
        } else {
            debug!("Server runtime stopped. Nothing to do.");
        }
    }

    pub async fn subchannel<STR: AsRef<str>>(
        &mut self,
        service_name: STR,
    ) -> Result<Arc<SubChannel>, TChannelError> {
        if let Some(subchannel) = self.subchannels.read().await.get(service_name.as_ref()) {
            return Ok(subchannel.clone());
        }
        self.make_subchannel(service_name).await
    }

    async fn make_subchannel<STR: AsRef<str>>(
        &self,
        service_name: STR,
    ) -> Result<Arc<SubChannel>, TChannelError> {
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
