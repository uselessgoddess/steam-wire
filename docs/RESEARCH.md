# Protobuf modernization research

> **Status: prost migration complete.** The whole workspace now generates and
> runs on **prost** (parsed by `protox`, no `protoc`, no C toolchain). The
> `rust-protobuf` 3.x backend and its runtime have been removed; the external
> rust-protobuf game crates were dropped during the migration and have since been
> **re-introduced as in-workspace prost crates** (`steam-vent-proto-{tf2,csgo}`,
> §8). Result on the real Steam proto surface:
> **594,019 → 58,773 lines of generated code (−90.1%)**, **23 MB → 2.6 MB on
> disk (−88.7%)**, and **110 generated files → 1**. CI builds the whole
> workspace `--locked` with zero warnings, all tests pass, clippy is clean, and
> the committed codegen is verified in sync with the `.proto` sources. The
> sections below record the analysis that led here and the final shape of the
> migration; §3.1 / §3.1.1 / §5-§6 are kept as the historical decision trail
> (now resolved) so the reasoning stays auditable.

## 1. Goal & constraints

From the issue, in priority order:

1. **Fundamental analysis**, then **either move to `prost` or update `protobuf`
   to 4+**. → **Moved to prost.**
2. **Maximum goal: minimal binary size + maximum build speed.** Evaluate
   `mold` + `sccache`. Add CI that *measures* build speed and binary size. →
   **Generated code cut ~10×; `mold`/`sccache`/`cargo-bloat` CI in place.**
3. **Update all Steam protobufs** if Steam changed them. → **Resynced to the
   latest SteamDatabase `Protobufs` snapshot (112 `.proto` files).**
4. **One Cargo workspace** so dependencies are easy to maintain and bump. →
   **Single `resolver = "3"`, edition-2024 workspace; `steam-totp` folded in.**

Everything below is measured against goals (1) and (2): whatever we picked had
to make the crate *smaller* and *faster to build*, not just "newer". prost does.

## 2. The Steam protobuf surface (measured, before vs after)

The dominant cost in this project — by a wide margin — is the *generated* code,
not the hand-written runtime (`steam-vent` is a few thousand lines on top). So
the headline metric of the migration is how much that generated code shrank:

| Metric | rust-protobuf 3.x (`lite_runtime`) | **prost (current)** | Δ |
| --- | ---: | ---: | ---: |
| `.proto` input files | 109 | 112¹ | — |
| Generated `.rs` files | 110 | **1** (`generated/mod.rs`) | −109 |
| Generated Rust LOC | 594,019 | **58,773** | **−90.1%** |
| Generated source on disk | 23 MB | **2.6 MB** | **−88.7%** |
| Largest single generated file | `htmlmessages.rs` — 1.16 MB | `mod.rs` — 2.6 MB² | — |

¹ The proto set was resynced to the latest SteamDatabase snapshot during the
migration, so the input counts differ slightly (109 → 112). For an
**identical-input** comparison see the proof-of-concept in §3.3, which measured
−83.6% LOC / −81.7% bytes on the *same* five protos before the full migration
was undertaken; the full-crate result (−90.1%) is even better because prost
collapses 110 per-file modules into one `mod.rs` and drops all per-file
boilerplate.

² prost emits a single module tree, so there is one file rather than 110 — it is
larger than any individual rust-protobuf file but ~9× smaller than their sum.

Steam's `.proto` files are **proto2** and carry **custom options** (per-message
`MsgId`, per-field descriptions, service routing names). These options are
consumed *at code-generation time* by `steam-vent-proto-build` to:

- map each message to its `EMsg` kind (`crates/steam-vent-proto-build/src/kinds.rs`), and
- emit the `Rpc{Service,Method,Message}` trait impls the protocol layer uses.

They are **not** needed at runtime — the fact that makes prost viable (§3.3).

## 3. Options evaluated

### 3.1 `rust-protobuf` 3.x (the previous status quo — stepancheg/rust-protobuf)

Crates: `protobuf`, `protobuf-codegen`, `protobuf-parse`. Pure Rust (no
`protoc`), full proto2 + custom-options support, `with-bytes` zero-copy fields.
Its weakness is exactly goal (2): **verbose codegen.** Every message gets
accessors, `CachedSize`, `SpecialFields`, reflection descriptors, etc. — the
594 KLOC above. `lite_runtime` (dropping reflection/text-format) was already on
and helped, but the remaining size/compile cost is intrinsic to its generated
struct shape. Only a framework change moved it further — which is what prost is.
**This backend has now been removed from the tree.**

#### 3.1.1 Historical note: the 3.5.1 pin (now moot)

During Phase 1 the workspace was pinned to `rust-protobuf =3.5.1` rather than the
then-latest 3.7.2, because the external game-proto crates
(`steam-vent-proto-{tf2,csgo,dota2}`) ship committed code hard-coding
`steam_vent_proto_common::protobuf::VERSION_3_5_1`, so they would not compile
against 3.7.2 (~40 errors in csgo, ~76 in dota2), and a `[patch.crates-io]`
deduped the graph to a single common crate. The prost migration made this entire
constraint moot: rust-protobuf is gone, the version pin is gone, the
`[patch.crates-io]` is gone, and the rust-protobuf game crates were dropped and
later re-created as prost crates (§8).

### 3.2 `protobuf` 4.x (Google's official Rust bindings) — **rejected**

- ❌ **Not pure Rust.** The default `upb` kernel is C (compiled via `cc`); the
  `cpp` kernel needs a full C++ `protobuf` install. Either way it pulls `protoc`
  and a C/C++ toolchain into the build graph.
- ❌ **Directly opposed to goal (2):** it *adds* heavy native build deps, hurting
  cold-build speed and reproducibility, and makes `mold`/`sccache` benefits
  marginal next to the C build.
- ❌ API still stabilizing; a hard break from the 3.x API with no migration path
  cheaper than prost, and no binary-size win to offset the toolchain cost.

**Conclusion:** of the two options the issue offered, "update to protobuf 4+" is
the wrong one for a project whose headline goal is *small + fast*. That left
**prost**.

### 3.3 `prost` (tokio-rs/prost) — **chosen and implemented**

- ✅ **Pure Rust at runtime.** Messages are plain structs with
  `#[derive(prost::Message)]`; no reflection, no per-field accessor wall, no
  cached-size/special-fields bookkeeping.
- ✅ **Dramatically more compact codegen** — the single biggest lever on both the
  generated LOC and the binary size, and the core reason prost wins here. This
  was first *measured, not assumed* in `experiments/prost-poc` (a representative
  five-proto subset): **−83.6% LOC / −81.7% bytes** on identical inputs. The
  full-crate migration then delivered **−90.1% / −88.7%** (§2).
- ✅ **Pure-Rust codegen too:** `protox` parses `.proto` files without `protoc`,
  feeding `prost-build`. Keeps the "no native toolchain" property.
- ✅ Optional fields, oneofs, nested messages, packed repeated, and
  `bytes::Bytes` fields are all supported.
- ✅ **proto2 custom options / extensions:** prost surfaces them as raw fields on
  the parsed `FileDescriptorSet`. Because Steam's options are only needed *at
  codegen time* (§2), `steam-vent-proto-build` reads them from the descriptors
  and needs no runtime extension support. This is the crux that makes prost
  viable for Steam.
- ✅ **Service traits:** prost emits messages only, and Steam RPC is not gRPC, so
  the existing custom `ServiceGenerator` in `steam-vent-proto-build` was retained
  and retargeted to emit `Rpc*` impls over prost structs.
- ✅ **Call-site churn — done.** The protocol layer (`net.rs`, `message.rs`,
  `game_coordinator/`, `auth/`, …) was ported off the rust-protobuf API:

  | rust-protobuf | prost |
  | --- | --- |
  | `msg.field()` getter | `msg.field` (`Option<T>` / `Vec<T>`) + `.unwrap_or(..)` |
  | `msg.set_field(v)` | `msg.field = Some(v)` |
  | `msg.compute_size()` | `msg.encoded_len()` |
  | `msg.write_to_writer(w)` | `msg.encode(w)` / `encode_to_vec` |
  | `T::parse_from_bytes(b)` | `T::decode(b)` |
  | `protobuf::Enum` + `.value()` | prost enums + `MsgKindEnum::enum_value()` |
  | `EMsg::k_EMsgFoo` | `EMsg::KEMsgFoo` (heck `UpperCamelCase`) |

## 4. Comparison matrix

| Criterion | rust-protobuf 3.x | protobuf 4.x (Google) | **prost (chosen)** |
| --- | --- | --- | --- |
| Pure-Rust build (no protoc/C) | ✅ (`.pure()`) | ❌ (upb/C++) | ✅ (`protox`) |
| Generated code size | ❌ 594 KLOC | ➖ medium | ✅ **58.7 KLOC** |
| Binary size | ➖ (✅ w/ lite) | ➖ | ✅ |
| Compile speed | ❌ | ❌ (+C build) | ✅ |
| proto2 + custom options | ✅ runtime | ✅ | ✅ codegen-time (sufficient) |
| Service traits | ⚙️ custom gen | ⚙️ | ⚙️ custom gen (retargeted) |
| Fits goal "small + fast" | partial | **no** | **yes** |

## 5. Decision (implemented)

1. **Rejected protobuf 4.x** — it adds a C/C++ + `protoc` build dependency, the
   opposite of the stated size/speed goal.
2. **Adopted prost** for the win on binary size and compile time, using `protox`
   so the build stays pure-Rust.
3. The work was staged so `main` always built — **both phases are now landed:**
   - **Phase 1:** single workspace; `steam-totp` added; `lite_runtime` codegen;
     size/speed profiles (`micro`, `nano`) and CI measuring build time + binary
     size with `mold` + `sccache`. (Delivered the workspace + measurement
     infrastructure with zero risk.)
   - **Phase 2 (this work):** prost generator in `steam-vent-proto-build`, the
     steam crate regenerated, the protocol-layer API ported, and the
     incompatible rust-protobuf game crates removed — then re-created as prost
     crates (§8). The §6 checklist below is the record of that phase, now fully
     checked off.

## 6. prost migration roadmap — **completed**

1. ✅ `protox` + `prost-build` in `steam-vent-proto-build`; parse `.proto`s to a
   `FileDescriptorSet` (no protoc).
2. ✅ Kind-mapping and RPC-trait generation reproduced by reading custom options
   from the descriptor set.
3. ✅ Runtime shims prost lacks but the protocol needs: `RpcMessage` /
   `RpcMessageWithKind` / `MsgKindEnum` traits over prost types, with a blanket
   `RpcMessage` impl for any `T: prost::Message + Default`
   (`crates/steam-vent-proto-common/src/lib.rs`).
4. ✅ `steam-vent-proto-steam` regenerated — LOC/size drop measured and recorded
   in §2.
5. ✅ `steam-vent` call sites ported (table in §3.3).
6. ✅ Build-time + binary-size CI (`build-metrics.yml`) retained to track the
   result over time.

## 7. Build-speed & binary-size strategy (goal 2)

Implemented at the workspace level (`Cargo.toml`):

- `profile.micro` — `release` + `codegen-units = 1` + `lto = "thin"` + `strip`.
- `profile.nano` — `micro` + `opt-level = "z"` + `lto = "fat"` + `panic = "abort"`.
- `profile.dev.build-override.opt-level = 3` so the proto **codegen**
  build-scripts/macros run optimized without slowing edit cycles.

Toolchain levers measured in CI (`.github/workflows/build-metrics.yml`):

- **`mold`** as the linker — the biggest single win on link time.
- **`sccache`** — caches `rustc` outputs across runs; turns the proto crate from
  a multi-minute cold build into a near-instant warm build.
- **`cargo-bloat`** — attributes binary size to crates/functions so the
  proto-vs-runtime split stays visible per run.

The job publishes wall-clock build time per profile and `cargo bloat` output for
the `login` example, so the size/speed story stays data-driven over time. (As a
local datapoint, the entire `steam-vent-proto-steam` crate plus its full prost
dependency stack now compiles cold in ~46 s; warm rebuilds are near-instant.)

## 8. Per-game game-coordinator crates (tf2/csgo) — **removed, then restored as prost crates**

### 8.1 Why they were removed

The optional `steam-vent-proto-{tf2,csgo,dota2}` *registry* crates were **dropped**
during the prost migration. They were **fundamentally incompatible with prost**:
each is generated by *rust-protobuf 3.x*, implements the old `protobuf::Message`
API, and links the rust-protobuf-era `steam-vent-proto-common` 0.5.1. Against the
new prost-based common 0.6.0 they fail to compile — the build surfaces
`there are multiple different versions of crate steam_vent_proto_common in the
dependency graph` and `CSOEconItem: prost::Message is not satisfied`. There was no
way to keep the *published* crates *and* finish the prost migration; they were
exact-pinned to the old world by construction. So they (plus the
`[patch.crates-io]` dedup hack that only existed to unify their common dep, the
`[features]` section, and the `backpack`/`inventory` examples) were removed to let
the migration land cleanly.

That removal was always meant to be temporary — the *protocol* never went away.
The whole **game-coordinator transport** is backend-agnostic and stayed in
`steam-vent` throughout: `Connection::game_coordinator`, `GenericGCHandshake`, the
`CMsgGcClient` envelope, and the GC message framing. Only the pre-generated
per-game payloads were gone.

### 8.2 How they were restored

`steam-vent-proto-tf2` and `steam-vent-proto-csgo` are now **in-workspace prost
crates**, regenerated from the upstream `.proto` sources
([SteamDatabase/Protobufs]) by the same `steam-vent-proto-build` pipeline that
generates the Steam crate (protox parse → prost-build codegen → committed
`src/generated/mod.rs`, verified in sync by CI). They link the prost-based
`steam-vent-proto-common` like every other crate here, so the incompatibility
above simply does not exist anymore. Each crate mirrors `steam-vent-proto-steam`:
a flat re-exported `generated` module plus a typed [`GCHandshake`] (app 440 for
tf2, 730 for csgo).

Two upstream files are intentionally **not** vendored per crate:
`enums_clientserver.proto` and `steammessages_base.proto`. The game `.proto`
files have **no `package`**, so prost merges every file compiled together into one
global namespace — and in proto2 that scope covers enum *value* names too. Those
two files reintroduce symbols that then collide (`EMsg::k_EMsgGCSystemMessage`
also exists as `EGCBaseMsg`/`ESOMsg` values; `CMsgProtoBufHeader` is defined in
both `steammessages.proto` and `steammessages_base.proto`), which protox rejects
as duplicate definitions. Neither file is imported by any game-coordinator proto,
and their general Steam-client types are already provided by
`steam-vent-proto-steam`, so dropping them is lossless. Every vendored set is the
full upstream game directory minus exactly those two files, with no dangling
imports.

They are wired into the `steam-vent-proto` facade behind off-by-default features
and re-exported under their own namespace (`steam_vent_proto::tf2`,
`steam_vent_proto::csgo`) rather than flattened — because the package-less game
protos reuse generic Steam-client symbol names, a flat merge would clash. The two
examples are back too: `backpack` (tf2) and `inventory` (csgo), gated behind
`required-features` so the (large) game modules are only built on demand.

`dota2` is **not** restored here — it was not requested, and unlike tf2/csgo it
has no example exercising it. It can be brought back the same way (vendor its
upstream protos minus the two collision files, add a crate mirroring tf2, wire a
feature) if a use case appears.

[SteamDatabase/Protobufs]: https://github.com/SteamDatabase/Protobufs
[`GCHandshake`]: https://docs.rs/steam-vent-proto-common/latest/steam_vent_proto_common/trait.GCHandshake.html
