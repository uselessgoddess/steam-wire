use std::io::{Read, Write};

use steam_vent_proto_common::{GCHandshake, ProtoError, RpcMessage, RpcMessageWithKind};
use steam_vent_proto_steam::CMsgClientHello;

use crate::game_coordinator::GCMsgKind;

pub struct GenericGCHandshake {
    pub app_id: u32,
    pub hello: CMsgClientHello,
}

impl GenericGCHandshake {
    #[must_use]
    pub fn new(app_id: u32) -> Self {
        Self { app_id, hello: CMsgClientHello::default() }
    }
}

impl GCHandshake for GenericGCHandshake {
    type Hello = CMsgClientHello;

    type Welcome = GenericCMsgClientWelcome;

    fn app_id(&self) -> u32 {
        self.app_id
    }

    fn hello(&self) -> Self::Hello {
        self.hello
    }
}

/// The welcome message a game-coordinator sends after a successful handshake.
///
/// The generic handshake doesn't care about the contents of the welcome, only
/// that it arrived, so this is modelled as an empty message: parsing drains and
/// discards the body and writing emits nothing.
#[derive(PartialEq, Clone, Default, Debug)]
pub struct GenericCMsgClientWelcome;

impl RpcMessage for GenericCMsgClientWelcome {
    fn parse(reader: &mut dyn Read) -> Result<Self, ProtoError> {
        // The payload is ignored, but the reader still has to be drained.
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf)?;
        Ok(GenericCMsgClientWelcome)
    }
    fn write(&self, _writer: &mut dyn Write) -> Result<(), ProtoError> {
        Ok(())
    }
    fn encode_size(&self) -> usize {
        0
    }
}

impl RpcMessageWithKind for GenericCMsgClientWelcome {
    type KindEnum = GCMsgKind;

    const KIND: Self::KindEnum = GCMsgKind::k_EMsgGCClientWelcome;
}
