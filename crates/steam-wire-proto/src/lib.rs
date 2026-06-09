//! Protobuf structs used by the Steam client protocol.
//!
//! This is a thin facade that re-exports the prost-generated Steam protobufs
//! ([`steam_wire_proto_steam`]) together with the shared transport traits
//! ([`steam_wire_proto_common`]) under one flat namespace, so downstream code
//! can write `steam_wire_proto::CMsgClientHello` regardless of which `.proto`
//! file a message originated from.
//!
//! The game-coordinator protobufs for individual games are re-exported under
//! their own sub-modules behind opt-in features: enable `tf2` for
//! [`steam_wire_proto::tf2`](tf2) (Team Fortress 2, app 440) and `csgo` for
//! [`steam_wire_proto::csgo`](csgo) (Counter-Strike 2 / CS:GO, app 730). They
//! are kept namespaced rather than flattened because the package-less game
//! protos reuse generic Steam-client symbol names. See `docs/RESEARCH.md` §8.

pub use steam_wire_proto_common::*;
pub use steam_wire_proto_steam::*;

#[cfg(feature = "csgo")]
pub use steam_wire_proto_csgo as csgo;
#[cfg(feature = "tf2")]
pub use steam_wire_proto_tf2 as tf2;
