use std::path::PathBuf;
fn main() {
    let protos_dir = PathBuf::from("../../crates/steam-vent-proto-steam/protos");
    let inputs = [
        "steammessages_base.proto",
        "enums_clientserver.proto", // defines EMsg
        "enums.proto",
        "steammessages_clientserver_login.proto",
        "steammessages_auth.steamclient.proto",
    ];
    let files: Vec<PathBuf> = inputs.iter().map(|f| protos_dir.join(f)).collect();
    let fds = protox::compile(&files, [&protos_dir]).expect("protox parse failed");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    // also dump the descriptor set so we can inspect services/options
    let mut cfg = prost_build::Config::new();
    cfg.out_dir(&out_dir);
    cfg.bytes(["."]);
    cfg.compile_fds(fds).expect("prost-build codegen failed");
    println!("cargo:warning=probe generated into {}", out_dir.display());
    // Copy generated file to a stable path for inspection.
    let gen = out_dir.join("_.rs");
    if gen.exists() {
        std::fs::copy(&gen, "generated_dump.rs").ok();
    }
}
