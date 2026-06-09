//! See build.rs — generation happens at build time into OUT_DIR. Including the
//! generated module here is the compile-proof: it shows prost's output for the
//! real Steam protos type-checks, not just that it generates. The measurement
//! reads the generated files out of OUT_DIR (see ../README.md).

// Steam protos declare no `package`, so prost emits the root package as `_.rs`.
mod steam {
    // The PoC only touches a couple of types; the rest are intentionally unused.
    #![allow(dead_code, clippy::all)]
    include!(concat!(env!("OUT_DIR"), "/_.rs"));
}

fn main() {
    // Touch a couple of generated types so they are not optimized away and the
    // binary genuinely links against the prost-generated code.
    // NB: prost normalizes message names to CamelCase (dropping the `_`
    // separators rust-protobuf keeps) — part of the call-site churn a real
    // migration would absorb.
    let req = steam::CContentServerDirectoryGetServersForSteamPipeRequest::default();
    let player = steam::CPlayerGetOwnedGamesRequest::default();
    println!(
        "prost-poc OK: built request types ({} cell_id, {:?} steamid)",
        req.cell_id.unwrap_or(0),
        player.steamid
    );
}
