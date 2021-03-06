use crate::handlers::RequestHandler;
use crate::messages::{Request, Response};
use std::collections::HashMap;

#[derive(Debug)]
pub struct TChannel {
    service_name: String,
}

impl TChannel {
    pub fn makeSubchannel<HANDLER: RequestHandler>(service_name: &str) -> SubChannel {
        SubChannel {
            service_name: service_name.to_string(),
            handlers: HashMap::new(),
        }
    }
}

pub struct TChannelBuilder {
    service_name: String,
}

impl TChannelBuilder {
    pub fn new(service_name: &str) -> TChannelBuilder {
        TChannelBuilder {
            service_name: String::from(service_name),
        }
    }

    pub fn build(self) -> TChannel {
        TChannel {
            service_name: self.service_name,
        }
    }
}

pub struct SubChannel {
    service_name: String,
    handlers: HashMap<String, Box<RequestHandler<REQ = Request, RES = Response>>>,
}

impl SubChannel {
    pub fn register<HANDLER: RequestHandler>(
        &mut self,
        handler_name: &str,
        handler: HANDLER,
    ) -> &Self {
        //TODO
        self
    }

    async fn send<REQ: Request, RES: Response>(request: REQ, host: String, port: u16) -> RES {
        unimplemented!()
    }
}
