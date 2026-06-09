//! Protobufs for steam-wire for Team Fortress 2 (app id 440) game-coordinator
//! messages.
//!
//! Like [`steam_wire_proto_steam`], the protobuf surface is generated and
//! consumed with [`prost`]: every message is a plain prost struct and the glue
//! ([`RpcMessageWithKind`]/[`MsgKindEnum`]) is emitted by
//! [`steam-wire-proto-build`]. The `.proto` sources have no `package`, so prost
//! collapses them into one flat [`generated`] module which is re-exported here.
//!
//! Send/receive these through the backend-agnostic game-coordinator transport in
//! `steam-wire` (`Connection::game_coordinator`); [`GCHandshake`] is the typed
//! handshake for app 440.
//!
//! Two upstream `.proto` files are intentionally **not** vendored here:
//! `enums_clientserver.proto` (the full Steam-client `EMsg`) and
//! `steammessages_base.proto`. Neither is imported by any tf2 game-coordinator
//! proto, both duplicate symbols (`EMsg::k_EMsgGCSystemMessage`,
//! `CMsgProtoBufHeader`) that collide once prost merges the package-less files
//! into a single module, and their general Steam-client types are already
//! provided by [`steam_wire_proto_steam`]. See `docs/RESEARCH.md` §8.
//!
//! [`steam_wire_proto_steam`]: https://docs.rs/steam-wire-proto-steam
//! [`prost`]: https://docs.rs/prost
//! [`steam-wire-proto-build`]: https://docs.rs/steam-wire-proto-build
//! [`RpcMessageWithKind`]: steam_wire_proto_common::RpcMessageWithKind
//! [`MsgKindEnum`]: steam_wire_proto_common::MsgKindEnum

mod generated;
mod handshake;

pub use generated::*;
pub use handshake::GCHandshake;
