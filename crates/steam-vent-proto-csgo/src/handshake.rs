use crate::{CMsgClientHello, CMsgClientWelcome};

/// The typed game-coordinator handshake for Counter-Strike 2 / Counter-Strike:
/// Global Offensive (app id 730).
///
/// Implements [`steam_vent_proto_common::GCHandshake`] with the csgo
/// `CMsgClientHello`/`CMsgClientWelcome` messages, so it can be passed straight
/// to `Connection::game_coordinator`.
#[derive(PartialEq, Clone, Default, Debug)]
pub struct GCHandshake {
    pub hello: CMsgClientHello,
}

impl steam_vent_proto_common::GCHandshake for GCHandshake {
    type Hello = CMsgClientHello;

    type Welcome = CMsgClientWelcome;

    fn app_id(&self) -> u32 {
        730
    }

    fn hello(&self) -> Self::Hello {
        self.hello.clone()
    }
}
