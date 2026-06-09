use crate::{CMsgClientHello, CMsgClientWelcome};

/// The typed game-coordinator handshake for Team Fortress 2 (app id 440).
///
/// Implements [`steam_wire_proto_common::GCHandshake`] with the tf2
/// `CMsgClientHello`/`CMsgClientWelcome` messages, so it can be passed straight
/// to `Connection::game_coordinator`.
#[derive(PartialEq, Clone, Default, Debug)]
pub struct GCHandshake {
    pub hello: CMsgClientHello,
}

impl steam_wire_proto_common::GCHandshake for GCHandshake {
    type Hello = CMsgClientHello;

    type Welcome = CMsgClientWelcome;

    fn app_id(&self) -> u32 {
        440
    }

    fn hello(&self) -> Self::Hello {
        // tf2's `CMsgClientHello` has a single scalar field, so prost derives
        // `Copy` for it and a plain copy is enough (csgo's carries a repeated
        // field and needs `.clone()` instead).
        self.hello
    }
}
