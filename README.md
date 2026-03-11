# Spear Data Normalization Gateway

A Rust workspace that normalizes legacy Middleware messages into protobuf
and publishes them to Redpanda. Designed for airgapped Kubernetes
deployment.

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  Build time                                                 │
│                                                             │
│   .xsd files ──► spear-gen ──► messages.proto               │
│                           └──► messages.rs (structs)        │
│                                     │                       │
│                                     ▼                       │
│                            compiled into                    │
│                            spear-gateway                    │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│  Runtime                                                    │
│                                                             │
│  Middleware ──WSDL/XML──► spear-gateway                     │
│                               │                            │
│                          spear-lib                         │
│                          ├ wsdl::extract_body_payload()     │
│                          ├ serde XML decode → Rust struct   │
│                          ├ prost encode → bytes             │
│                          ├ ProtoEnvelope wrap               │
│                          └ Publisher → Redpanda topic       │
│                                                             │
│  HTTP control plane:                                        │
│    POST /subscribe   → starts adapter                       │
│    POST /unsubscribe → stops adapter                        │
└─────────────────────────────────────────────────────────────┘
```

---

## Crates

| Crate | Type | Purpose |
|---|---|---|
| `spear-gen` | binary | Code generator: XSD → `.proto` + `.rs` |
| `spear-lib` | library | Runtime: WSDL parser, `ProtoEnvelope`, Redpanda publisher |
| `spear-gateway` | binary | HTTP control plane + adapter runner (in progress) |

---

## Prerequisites

- Rust toolchain (`rustup` recommended, stable)
- `cmake` (required by `rdkafka`'s bundled librdkafka build)
- For musl static builds: Docker (see [Deployment](#deployment))

---

## Building

```bash
# Build everything
cargo build

# Build only the generator
cargo build -p spear-gen

# Run tests
cargo test
```

---

## spear-gen: XSD → code generation

Takes a directory of `.xsd` files and emits:

- `messages.proto` — proto3 schema for downstream consumers
- `messages.rs` — Rust structs with both XML (`serde`) and protobuf
  (`prost`) derives, used by the gateway to decode and re-encode messages

```bash
cargo run -p spear-gen -- \
  --input schemas/synthetic \
  --out-proto generated/proto \
  --out-rust generated/rust
```

Output files are regenerated whenever the XSD inputs change. On the
classified side, point `--input` at the real XSD directory.

See [docs/xsd-proto-mapping.md](docs/xsd-proto-mapping.md) for the full
mapping rules and known limitations.

---

## Development with synthetic schemas

The `schemas/synthetic/` directory contains three representative XSD
files used for local development and testing:

| File | Demonstrates |
|---|---|
| `track.xsd` | Nested complex types, optional fields, enumerations |
| `alert.xsd` | `xs:choice`, `maxOccurs="unbounded"`, cross-file enum refs |
| `status.xsd` | `xs:extension` (inheritance), 3-level nesting, repeated fields |

These cover all XSD patterns the parser handles. The classified-side XSDs
drop in as a direct replacement.

---

## Deployment

Binaries are built as musl static binaries for airgapped Kubernetes
deployment — no shared library dependencies, no external registry access
required at runtime.

The Docker build takes XSD files as input and produces a self-contained
gateway image:

```bash
# Build with synthetic schemas (dev)
docker build --build-arg XSD_DIR=schemas/synthetic \
  -f docker/Dockerfile.gateway -t spear-gateway .

# Build with real schemas (classified side)
docker build --build-arg XSD_DIR=schemas/real \
  -f docker/Dockerfile.gateway -t spear-gateway .
```

The `spear-gen` binary is also available as a standalone musl binary for
running code generation directly without Docker.

---

## Project phases

| Phase | Status | Description |
|---|---|---|
| Phase 1 | In progress | `spear-gen` + `spear-lib` |
| Phase 2 | Planned | Gateway HTTP control plane |
| Phase 3 | Planned | Middleware adapter (decode → normalize → Redpanda) |
| Phase 4 | Planned | Hardening, observability, airgapped K8s |
