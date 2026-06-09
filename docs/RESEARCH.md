# Protobuf modernization research

> Status: living document. Phase 1 (workspace consolidation + green build on
> `rust-protobuf` 3.7.x) is implemented in this PR. The prost migration is
> scoped here and tracked as follow-up work so the tree never regresses.

## 1. Goal & constraints

From the issue, in priority order:

1. **Fundamental analysis**, then **either move to `prost` or update `protobuf`
   to 4+**.
2. **Maximum goal: minimal binary size + maximum build speed.** Evaluate
   `mold` + `sccache`. Add CI that *measures* build speed and binary size.
3. **Update all Steam protobufs** if Steam changed them.
4. **One Cargo workspace** so dependencies are easy to maintain and bump.

Everything below is measured against goals (1) and (2): whatever we pick must
make the crate *smaller* and *faster to build*, not just "newer".

## 2. The Steam protobuf surface (measured)

Numbers from `crates/steam-vent-proto-steam` in this repo:

| Metric | Value |
| --- | --- |
| `.proto` input files | 109 |
| Generated `.rs` files | 110 |
| Generated Rust LOC | **594,019** |
| Generated source on disk | **23 MB** |
| Largest single generated file | `htmlmessages.rs` — 1.16 MB |

This 594 KLOC of generated code is, by a wide margin, the dominant cost in both
**compile time** and **binary size**. It is the thing to optimize. The runtime
crate (`steam-vent`) is a few thousand lines on top.

Steam's `.proto` files are **proto2** and carry **custom options**
(per-message `MsgId`, per-field descriptions, service routing names). These
options are consumed *at code-generation time* by `steam-vent-proto-build` to:

- map each message to its `EMsg` kind (see `crates/steam-vent-proto-build/src/kinds.rs`), and
- emit the `Rpc{Service,Method,Message}` trait impls that the protocol layer uses.

They are **not** needed at runtime — an important fact for the prost analysis.

## 3. Options evaluated

### 3.1 `rust-protobuf` 3.x (status quo — stepancheg/rust-protobuf)

Crates: `protobuf`, `protobuf-codegen`, `protobuf-parse`. The repo pins
`=3.5.1`; latest stable is **3.7.2** (verified on crates.io).

- ✅ **Pure Rust**, no `protoc`, no C toolchain. Codegen runs via
  `protobuf-parse`'s `.pure()` parser — exactly what the in-repo
  `steam-vent-proto-build` tool already does.
- ✅ Full **proto2 + extensions/custom-options** support, which the build tool
  relies on to derive `EMsg` kinds and RPC traits.
- ✅ `with-bytes` gives zero-copy `Bytes` fields (already enabled).
- ⚠️ **Verbose codegen.** Every message gets accessors, `CachedSize`,
  `SpecialFields`, reflection descriptors, `oneof` enums, etc. → the 594 KLOC
  above. Reflection/descriptor data also inflates the binary.
- ➖ **Mitigation that helps today:** `Codegen::lite_runtime(true)` /
  `optimize_for = LITE_RUNTIME` drops reflection and `Debug`/text-format
  machinery. This is the cheapest available size/compile win *without changing
  the framework* and is compatible with the existing protocol code (it only
  uses the binary wire API: `parse_from_bytes`, `write_to_writer`,
  `compute_size`, field accessors, the `Enum` trait).

### 3.2 `protobuf` 4.x (Google's official Rust bindings) — **rejected**

- ❌ **Not pure Rust.** The default `upb` kernel is C, compiled via `cc`; the
  `cpp` kernel needs a full C++ `protobuf` install. Either way it pulls
  `protoc` and a C/C++ toolchain into the build graph.
- ❌ This is **directly opposed to goal (2)**: it *adds* heavy native build
  dependencies, hurting both cold-build speed and reproducibility, and makes
  `mold`/`sccache` benefits marginal next to the C build.
- ❌ API is still stabilizing and is a hard break from the 3.x API the protocol
  layer uses; no migration path that is cheaper than prost.
- ❌ No real binary-size advantage to offset the toolchain cost.

**Conclusion:** of the two options the issue offered, "update to protobuf 4+"
is the wrong one for a project whose headline goal is *small + fast*. That
leaves **prost**.

### 3.3 `prost` (tokio-rs/prost) — **recommended target**

- ✅ **Pure Rust at runtime.** Messages are plain structs with
  `#[derive(prost::Message)]`; no reflection, no per-field accessor wall, no
  cached-size/special-fields bookkeeping in the public API.
- ✅ **Dramatically more compact codegen** → the single biggest lever on both
  the 594 KLOC and the binary size. This is the core reason prost wins here.
- ✅ Codegen can be **pure Rust** too: `protox` parses `.proto` files without
  `protoc`, feeding `prost-build`'s `Config::compile_fds`. Keeps the
  "no native toolchain" property we already have.
- ✅ Optional fields, oneofs, nested messages, packed repeated, `bytes::Bytes`
  fields (`bytes(".")`) are all supported.
- ⚠️ **proto2 custom options / extensions**: prost surfaces unknown options as
  raw fields on the `FileDescriptorSet` rather than typed extensions. Because
  Steam's options are only needed *at codegen time* (§2), we read them from the
  parsed descriptors in our own generator and **do not** need runtime extension
  support. This is the crux that makes prost viable for Steam.
- ⚠️ **No service traits out of the box.** prost emits messages only. Steam RPC
  is not gRPC, so we keep our own service-trait generator (the existing
  `ServiceGenerator` logic in `steam-vent-proto-build`), retargeted to emit
  impls over prost structs instead of `protobuf::Message`.
- ⚠️ **API churn at the call sites.** The protocol layer
  (`net.rs`, `message.rs`, …) uses the rust-protobuf API: generated getters
  (`.jobid_source()`), setters (`.set_jobid_source()`), `.compute_size()`,
  `.write_to_writer()`, `parse_from_bytes`, `CMsgMulti::parse_from_reader`,
  the `Enum` trait, `with-bytes`. prost replaces these with public struct
  fields, `Message::encode/encoded_len/decode`. Every touchpoint must be
  ported. This is the real cost of the migration and why it is phased.

## 4. Comparison matrix

| Criterion | rust-protobuf 3.x | protobuf 4.x (Google) | prost |
| --- | --- | --- | --- |
| Pure-Rust build (no protoc/C) | ✅ (`.pure()`) | ❌ (upb/C++) | ✅ (`protox`) |
| Generated code size | ❌ very large | ➖ medium | ✅ compact |
| Binary size | ➖ (✅ w/ lite) | ➖ | ✅ |
| Compile speed | ❌ | ❌ (+C build) | ✅ |
| proto2 + custom options | ✅ runtime | ✅ | ⚠️ codegen-time only (sufficient) |
| Service traits | ⚙️ custom gen | ⚙️ | ⚙️ custom gen |
| Migration cost from today | none | high | medium |
| Fits goal "small + fast" | partial | **no** | **yes** |

## 5. Decision

1. **Reject protobuf 4.x** — it adds a C/C++ + `protoc` build dependency, the
   opposite of the stated size/speed goal.
2. **Target prost** for the long-term win on binary size and compile time,
   using `protox` so the build stays pure-Rust.
3. **Phase the work** so `main` always builds:
   - **Phase 1 (this PR):** single workspace; vendored proto crates building on
     `rust-protobuf` 3.7.x; `steam-totp` added; size/speed profiles (`micro`,
     `nano`) and CI that measures build time + binary size with `mold` +
     `sccache`. Optionally flip codegen to `lite_runtime` for an immediate size
     cut. This delivers measurable progress against goal (2) with zero risk.
   - **Phase 2 (follow-up):** introduce a `prost`-based generator in
     `steam-vent-proto-build` behind a feature, regenerate the steam crate,
     port the protocol-layer API touchpoints, and compare binary size / build
     time head-to-head against Phase 1 using the same CI job.

## 6. prost migration roadmap (Phase 2)

1. Add `protox` + `prost-build` to `steam-vent-proto-build`; parse `.proto`s to
   a `FileDescriptorSet` (no protoc).
2. Reproduce kind-mapping and RPC-trait generation by reading custom options
   from the descriptor set (already the tool's job, different input type).
3. Define the runtime shims prost lacks but the protocol needs: a thin
   `Message` wrapper exposing `encode`/`decode`/`encoded_len`, and the
   `RpcMessage`/`MsgKindEnum` traits over prost types.
4. Regenerate `steam-vent-proto-steam`; expect a large LOC/size drop (re-measure
   and record in §2).
5. Port `steam-vent` call sites: accessors→fields, `compute_size`→`encoded_len`,
   `write_to_writer`→`encode`, `parse_from_bytes`→`decode`, `Enum`→prost enums.
6. Run the binary-size + build-time CI on both branches; keep prost only if it
   wins (it is expected to).

## 7. Build-speed & binary-size strategy (goal 2)

Implemented at the workspace level (`Cargo.toml`):

- `profile.micro` — `release` + `codegen-units = 1` + `lto = "thin"` + `strip`.
- `profile.nano` — `micro` + `opt-level = "z"` + `lto = "fat"` + `panic = "abort"`.
- `profile.dev.build-override.opt-level = 3` so the heavy proto **codegen**
  build scripts/macros run optimized without slowing edit cycles.

Toolchain levers measured in CI (see `.github/workflows/`):

- **`mold`** as the linker — biggest single win on link time for a crate that
  links 594 KLOC of generated objects.
- **`sccache`** — caches `rustc` outputs across CI runs; turns the proto crate
  from a multi-minute cold build into a near-instant warm build.
- **`cargo-bloat`** — attributes binary size to crates/functions so the
  proto-vs-runtime split (and prost's effect on it) is visible per run.

The CI publishes wall-clock build time per profile and `cargo bloat` output so
the prost-vs-rust-protobuf comparison in §5/§6 is data-driven, not assumed.
