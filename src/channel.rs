use crate::connection::pool::{ConnectionPools, ConnectionPoolsBuilder};
use crate::connection::ConnectionOptions;
use crate::errors::TChannelError;
use crate::SubChannel;
use log::debug;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct TChannel {
    subchannels: RwLock<HashMap<String, Arc<SubChannel>>>,
    connection_pools: Arc<ConnectionPools>,
}

impl TChannel {
    pub fn new(connection_options: ConnectionOptions) -> Result<Self, TChannelError> {
        let connection_pools = ConnectionPoolsBuilder::default()
            .connection_options(connection_options)
            .build()?;
        Ok(TChannel {
            subchannels: RwLock::new(HashMap::new()),
            connection_pools: Arc::new(connection_pools),
        })
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
