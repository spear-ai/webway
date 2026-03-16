# Spear Data Normalization Gateway

A Rust workspace that decodes legacy binary messages using types generated
from XSD schemas or C header files. Designed for airgapped deployment via a
pre-built, fully-vendored container image.

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  Code generation                                            │
│                                                             │
│   .xsd files  ──► spear-gen  ──► types.proto                │
│                           └──► types.rs                     │
│                                (decode_raw / encoded_size)  │
│                                                             │
│   .h files    ──► header-gen ──► structs.rs  (decode())     │
│                           ├──► messages.proto               │
│                           ├──► mappers.rs                   │
│                           └──► review_report.txt            │
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
| `header-gen` | binary | Code generator: C headers → Rust structs + `.proto` + mapping functions |
| `spear-lib` | library | Runtime: WSDL parser, `ProtoEnvelope`, Redpanda publisher |
| `spear-gateway` | binary | Decode pipeline: raw binary bytes → generated types → printed output |

---

## Building locally

```bash
# Requires: Rust stable, cmake, libcurl (for rdkafka in spear-lib)
cargo build --workspace --exclude header-gen
cargo test --workspace --exclude header-gen

# header-gen additionally requires llvm-dev + libclang-dev (Linux) or
# brew install llvm (macOS). Build and test it separately:
DYLD_LIBRARY_PATH=/opt/homebrew/opt/llvm/lib cargo test -p header-gen  # macOS
cargo test -p header-gen                                                  # Linux
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

## header-gen: C header → code generation

Takes a directory of `.h` files and emits three files per struct:

- `--out-rust` — Rust structs with `decode(bytes: &[u8])` (offset-based, configurable endianness) + `review_report.txt` for anything requiring manual review (bitfields, unions, unresolved types)
- `--out-proto` — proto3 message definitions
- `--out-mapping` — explicit `map_*()` functions from each Rust struct to its proto message

**From the release binary** (no system libclang install required):

```bash
# Download from GitHub Releases and extract.
# The tarball contains three files: the binary, libclang-XX.so.XX, and libLLVM.so.XX.
# Keep them all in the same directory — the binary finds them via RPATH=$ORIGIN.
tar -xzf header-gen-linux-x86_64-vX.Y.Z.tar.gz
cd header-gen-linux-x86_64-vX.Y.Z/   # or wherever it extracted to

./header-gen \
  --input      headers/ \
  --endian     little \
  --word-size  32 \
  --define     LINUX \
  --out-rust   generated/rust \
  --out-proto  generated/proto \
  --out-mapping generated/mapping
```

**From source** (requires Rust + libclang):

```bash
cargo run -p header-gen -- \
  --input      headers/ \
  --endian     little \
  --word-size  32 \
  --define     LINUX \
  --out-rust   generated/rust \
  --out-proto  generated/proto \
  --out-mapping generated/mapping
```

`--word-size` controls how `long`/`unsigned long` are mapped:
- `32` → `i32`/`u32` (LP32/ILP32 ABI)
- `64` → `i64`/`u64` (LP64 ABI)

`--endian` controls the decode method emitted (`from_le_bytes` vs `from_be_bytes`).

**Binary distribution:** the release tarball bundles `libclang.so` and `libLLVM.so` alongside the binary. RPATH is set to `$ORIGIN` on both the binary and `libclang.so` so they find their dependencies in the same directory — no system LLVM installation required. Ubuntu does not ship the monolithic `libclang.a` needed for fully-static linking, so bundling the shared libraries is the most practical approach.

```bash
./scripts/build-header-gen.sh
# → target/release/header-gen + libclang-XX.so.XX + libLLVM.so.XX (copy all three to the target machine)
```

---

## Synthetic schemas

`schemas/synthetic/` contains representative XSD files used for local
development and CI. The classified-side XSDs drop in as a direct
replacement.

| File | Demonstrates |
|---|---|
| `track.xsd` | Nested complex types, optional fields, enumerations |
| `alert.xsd` | `xs:choice`, `maxOccurs="unbounded"`, cross-file enum refs |
| `status.xsd` | `xs:extension` (inheritance), 3-level nesting, plain string enums |
| `sub/credentials.xsd` | Subdirectory scanning, primitive type aliases (`xs:base64Binary` → `Vec<u8>`) |

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
| `test` | `cargo test --workspace --exclude header-gen` on Ubuntu + macOS |
| `test-header-gen` | `cargo test -p header-gen` on Ubuntu (requires libclang) |
| `check-musl` | `cargo check -p spear-gen` (musl target) |
| `lint` | `cargo fmt --check` + `cargo clippy -D warnings` (full workspace) |

`header-gen` is excluded from the cross-platform `test` matrix because macOS
runners don't ship with `libclang`. It gets full test coverage in the dedicated
`test-header-gen` job on Linux.

Releases (tagged `v*`) build and attach to a GitHub Release:
- `spear-gen-linux-x86_64-musl.tar.gz` — airgapped deployment binary (musl static)
- `spear-gen-linux-x86_64.tar.gz`
- `spear-gen-macos-arm64.tar.gz`
- `spear-gen-macos-x86_64.tar.gz`
- `header-gen-linux-x86_64.tar.gz` — static LLVM, no runtime libclang dep

---

## Project phases

| Phase | Status | Description |
|---|---|---|
| Phase 1 | Done | `spear-gen` (XSD → proto + Rust) + `spear-lib` (envelope, publisher, WSDL parser) |
| Phase 2 | Done | `spear-gateway` decode pipeline + offline dev container (`spear-dev`) |
| Phase 2b | Done | `header-gen` (C headers → Rust structs + proto + mapping functions) |
| Phase 3 | Planned | Live legacy broker integration → normalize → publish to Redpanda |
| Phase 4 | Planned | Hardening, observability, airgapped K8s manifests |
