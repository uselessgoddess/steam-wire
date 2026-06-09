//! Generate a representative subset of the Steam protobuf surface with prost,
//! parsing the `.proto` sources with `protox` (pure Rust, no `protoc`).
//!
//! The point is measurement, not production: we want the generated-code size on
//! the prost side for the same inputs that produce
//! `crates/steam-vent-proto-steam/src/generated/{steammessages_base,
//! steammessages_unified_base_steamclient, enums,
//! steammessages_contentsystem_steamclient, steammessages_player_steamclient}.rs`
//! on the rust-protobuf side.
use std::path::PathBuf;

fn main() {
    let protos_dir = PathBuf::from("../../crates/steam-vent-proto-steam/protos");

    // Representative subset: the two base/option-defining files, the shared enum
    // file, and two real service protos (one small, one large).
    let inputs = [
        "steammessages_base.proto",
        "steammessages_unified_base.steamclient.proto",
        "enums.proto",
        "steammessages_contentsystem.steamclient.proto",
        "steammessages_player.steamclient.proto",
    ];
    let files: Vec<PathBuf> = inputs.iter().map(|f| protos_dir.join(f)).collect();

    for f in &files {
        println!("cargo:rerun-if-changed={}", f.display());
    }

    // protox parses proto2 + Steam's custom options (declared in the base files
    // we include) into a FileDescriptorSet without protoc.
    let fds = protox::compile(&files, [&protos_dir]).expect("protox parse failed");

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let mut cfg = prost_build::Config::new();
    cfg.out_dir(&out_dir);
    // Zero-copy bytes, mirroring rust-protobuf's `with-bytes` in the real crate.
    cfg.bytes(["."]);
    cfg.compile_fds(fds).expect("prost-build codegen failed");

    println!("cargo:warning=prost-poc generated into {}", out_dir.display());
}
