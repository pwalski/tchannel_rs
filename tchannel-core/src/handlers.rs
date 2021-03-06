use crate::messages::{Request, Response};
use crate::Result;

pub trait RequestHandler {
    type REQ: Request;
    type RES: Response;

    fn handle(self: &mut Self, request: Self::REQ) -> Result<Self::RES>;
}
