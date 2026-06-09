//! Protobuf structs used by the Steam client protocol.
//!
//! This is a thin facade that re-exports the prost-generated Steam protobufs
//! ([`steam_vent_proto_steam`]) together with the shared transport traits
//! ([`steam_vent_proto_common`]) under one flat namespace, so downstream code
//! can write `steam_vent_proto::CMsgClientHello` regardless of which `.proto`
//! file a message originated from.
//!
//! The tf2/csgo/dota2 game-coordinator crates that used to be re-exported here
//! were dropped in the prost migration (they are rust-protobuf 3.x and cannot
//! link against the prost-based common). See `docs/RESEARCH.md`.

pub use steam_vent_proto_common::*;
pub use steam_vent_proto_steam::*;
