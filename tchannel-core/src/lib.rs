pub mod tchannel {
    use std::io;

    fn newChannel(name: String) -> Result<Channel, io::Error> {
        let peers = PeerList{};
        let consectionOptions = ConnectionOptions{};
        let createdStack = String::from("stack");
        let channel = Channel{id: 1, createdStack, consectionOptions, peers};
        Ok(channel)
    }

    pub struct Channel {
        id: u32,
        createdStack: String,
        consectionOptions: ConnectionOptions,
        peers: PeerList
    }

    pub struct ConnectionOptions {

    }

    pub struct PeerList {

    }

    pub struct Handler {

    }

    pub enum ConnectionDirection {
        None,
        In,
        Out
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn functional_test() {
        // - new channel for client

        // - register handler to channel

        // - add peer using addr string
        // - create subchannel using serviceName string
        // - create client using channel, subchannel, clientoptions, and serviceName
        assert_eq!(2 + 2, 4);
    }
}
