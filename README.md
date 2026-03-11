# Spear Data Normalization Gateway

A Rust workspace that decodes legacy Middleware binary messages using types
generated from XSD schemas. Designed for airgapped deployment via a
pre-built, fully-vendored container image.

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  Code generation (spear-gen)                                │
│                                                             │
│   .xsd files ──► spear-gen ──► types.proto                  │
│                           └──► types.rs                     │
│                                (decode_raw / encoded_size)  │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│  Decode pipeline (spear-gateway)                            │
│                                                             │
│  legacy broker ──raw binary──► decode_raw() → Rust struct   │
│       or                                                    │
│  captured file ──raw binary──► decode_raw() → Rust struct   │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│  Future: normalization + publish (spear-lib)                │
│                                                             │
│  decoded struct                                             │
│      │                                                      │
│      ▼                                                      │
│  prost encode → ProtoEnvelope → Publisher → Redpanda        │
│                                                 │           │
│                                        new consumers        │
└─────────────────────────────────────────────────────────────┘
```

---

## Crates

| Crate | Type | Purpose |
|---|---|---|
| `spear-gen` | binary | Code generator: XSD → `.proto` + `.rs` with `decode_raw`/`encoded_size` |
| `spear-lib` | library | Runtime: WSDL parser, `ProtoEnvelope`, Redpanda publisher |
| `spear-gateway` | binary | Decode pipeline: raw binary bytes → generated types → printed output |

---

## Building locally

```bash
# Requires: Rust stable, cmake, libcurl (for rdkafka in spear-lib)
cargo build
cargo test
```

---

## spear-gen: XSD → code generation

Takes a directory of `.xsd` files and emits:

- `--out-proto` — proto3 schema for downstream consumers
- `--out-rust` — Rust structs with `decode_raw(buf, same_endianness)` and
  `encoded_size()` for the legacy binary wire format, plus `prost::Message`
  and `serde::Deserialize` derives

```bash
cargo run -p spear-gen -- \
  --input     schemas/synthetic \
  --out-proto generated/types.proto \
  --out-rust  generated/types.rs
```

See [docs/xsd-proto-mapping.md](docs/xsd-proto-mapping.md) for the full
XSD → proto3/Rust mapping rules.

---

## Synthetic schemas

`schemas/synthetic/` contains three representative XSD files used for
local development and CI. The classified-side XSDs drop in as a direct
replacement.

| File | Demonstrates |
|---|---|
| `track.xsd` | Nested complex types, optional fields, enumerations |
| `alert.xsd` | `xs:choice`, `maxOccurs="unbounded"`, cross-file enum refs |
| `status.xsd` | `xs:extension` (inheritance), 3-level nesting, plain string enums |

---

## Airgapped deployment

The classified side has no crate registry. The workflow is:

### 1. Build the dev container (internet-connected machine)

```bash
./scripts/build-image.sh
# → vendors all crates, builds linux/amd64 image, saves to spear-dev.tar.gz
```

Transfer `spear-dev.tar.gz` to the classified side.

### 2. Load and run the container (classified side)

```bash
podman load < spear-dev.tar.gz

podman run -d --name spear-dev \
  -v /path/to/workspace:/workspace \
  spear-dev:latest

podman exec -it spear-dev bash
```

The container has the full Rust toolchain, all vendored crate sources, and
pre-compiled build artifacts. Rebuilds inside the container only recompile
changed Rust — the heavy C dependencies are already done.

### 3. Generate types from real XSDs (inside container)

```bash
spear-gen \
  --input     /workspace/xsds \
  --out-proto /workspace/types.proto \
  --out-rust  /workspace/types.rs
```

### 4. Plug in generated types and rebuild

```bash
cp /workspace/types.rs /spear/crates/spear-gateway/src/types.rs
# In crates/spear-gateway/src/main.rs:
#   1. Uncomment include!("types.rs")
#   2. Add decode_raw call in decode_and_print()
cargo build --offline --release -p spear-gateway
```

### 5. Decode a captured binary

```bash
# File mode — decode a raw binary captured from the wire
./target/release/spear-gateway --file /workspace/captures/msg.bin

# Live mode — connect to the legacy broker (C integration, coming later)
./target/release/spear-gateway --live
```

---

## CI

| Job | What it checks |
|---|---|
| `test` | `cargo test --workspace` on ubuntu + macos |
| `check-musl` | `cargo check -p spear-gen` (musl target) |
| `lint` | `cargo fmt --check` + `cargo clippy -D warnings` |

Releases (tagged `v*`) build musl and native binaries for linux and macOS
and attach them to a GitHub Release.

---

## Project phases

| Phase | Status | Description |
|---|---|---|
| Phase 1 | Done | `spear-gen` (XSD → proto + Rust) + `spear-lib` (envelope, publisher, WSDL parser) |
| Phase 2 | Done | `spear-gateway` decode pipeline + offline dev container (`spear-dev`) |
| Phase 3 | Planned | Live legacy broker integration → normalize → publish to Redpanda |
| Phase 4 | Planned | Hardening, observability, airgapped K8s manifests |
