# Spear Data Normalization Gateway

A Rust workspace that normalizes legacy Middleware messages into protobuf
and publishes them to Redpanda. Designed for airgapped deployment via a
pre-built, fully-vendored container image.

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  Code generation (spear-gen)                                │
│                                                             │
│   .xsd files ──► spear-gen ──► messages.proto               │
│                           └──► messages.rs                  │
│                                (decode_raw / encoded_size)  │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│  Runtime (spear-gateway)                                    │
│                                                             │
│  Redpanda topic                                             │
│      │                                                      │
│      ▼                                                      │
│  ProtoEnvelope::decode_from_bytes()   (spear-lib)           │
│      │                                                      │
│      ▼                                                      │
│  <GeneratedType>::decode_raw()        (from spear-gen)      │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│  Publishing path (spear-lib)                                │
│                                                             │
│  Middleware ──WSDL/XML──► wsdl::extract_body_payload()      │
│                               │                            │
│                          serde XML decode → Rust struct     │
│                          prost encode → bytes               │
│                          ProtoEnvelope::new()               │
│                          Publisher::publish() → Redpanda    │
└─────────────────────────────────────────────────────────────┘
```

---

## Crates

| Crate | Type | Purpose |
|---|---|---|
| `spear-gen` | binary | Code generator: XSD → `.proto` + `.rs` with `decode_raw`/`encoded_size` |
| `spear-lib` | library | Runtime: WSDL parser, `ProtoEnvelope`, Redpanda publisher |
| `spear-gateway` | binary | Redpanda consumer; decodes `ProtoEnvelope` and dispatches to generated types |

---

## Building locally

```bash
# Requires: Rust stable, cmake, libcurl (for rdkafka)
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
  --input   schemas/synthetic \
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
pre-compiled build artifacts. `rdkafka`'s C build is already done in the
image — rebuilds on classified only recompile changed Rust.

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
# Uncomment include!("types.rs") in crates/spear-gateway/src/main.rs
# Add decode_raw call in handle_message()
cargo build --offline --release -p spear-gateway
```

### 5. Run against Redpanda

```bash
REDPANDA_BROKERS=redpanda-host:9092 \
SPEAR_TOPIC=spear.messages \
  ./target/release/spear-gateway
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
| Phase 2 | Done | `spear-gateway` consumer skeleton + offline dev container (`spear-dev`) |
| Phase 3 | Planned | Middleware adapter: live WSDL ingest → normalize → Redpanda |
| Phase 4 | Planned | Hardening, observability, airgapped K8s manifests |
