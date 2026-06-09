#!/usr/bin/env bash
# Reproducible prost-vs-rust-protobuf generated-code size comparison on a
# representative subset of the Steam protobuf surface.
#
# Usage:  ./measure.sh
#
# Builds the prost PoC (parsing with protox — pure Rust, no protoc), then
# compares the generated LOC/bytes against the committed rust-protobuf output
# in crates/steam-vent-proto-steam/src/generated for the SAME five protos.
set -euo pipefail
cd "$(dirname "$0")"

echo "==> Building prost PoC (protox parse + prost-build codegen + compile)…"
cargo run --quiet >/dev/null

PROST_OUT="$(find target/*/build/prost-poc-*/out -name '_.rs' | head -1)"
GEN="../../crates/steam-vent-proto-steam/src/generated"
RP_FILES=(
  "$GEN/steammessages_base.rs"
  "$GEN/steammessages_unified_base_steamclient.rs"
  "$GEN/enums.rs"
  "$GEN/steammessages_contentsystem_steamclient.rs"
  "$GEN/steammessages_player_steamclient.rs"
)

prost_loc=$(wc -l < "$PROST_OUT");           prost_bytes=$(wc -c < "$PROST_OUT")
rp_loc=$(cat "${RP_FILES[@]}" | wc -l);       rp_bytes=$(cat "${RP_FILES[@]}" | wc -c)

printf '\n%-16s %12s %14s\n' "backend" "LOC" "bytes"
printf '%-16s %12s %14s\n'   "rust-protobuf" "$rp_loc" "$rp_bytes"
printf '%-16s %12s %14s\n'   "prost"         "$prost_loc" "$prost_bytes"
awk -v p="$prost_loc"   -v r="$rp_loc"   'BEGIN{printf "\nLOC reduction:   %.1f%%\n", (1-p/r)*100}'
awk -v p="$prost_bytes" -v r="$rp_bytes" 'BEGIN{printf "Bytes reduction: %.1f%%\n", (1-p/r)*100}'
