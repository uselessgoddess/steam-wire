//! Protobufs for steam-wire for non-game messages

mod generated;

pub use generated::*;

impl steam_wire_proto_common::JobMultiple for CMsgClientPicsProductInfoResponse {
    fn completed(&self) -> bool {
        !self.response_pending.unwrap_or(false)
    }
}
