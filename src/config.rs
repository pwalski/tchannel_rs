use core::default::Default;
use core::option::Option::Some;
use core::time::Duration;
use std::net::SocketAddr;
use std::net::{IpAddr, Ipv4Addr};

/// TChannel config
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
            server_address: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8888),
            server_tasks: 4,
        }
    }
}
