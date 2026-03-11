//! # spear-gen
//!
//! Code generator that converts XSD schema files into two outputs:
//!
//! - **`.proto` files** (proto3) — structural mirrors of the XSD definitions,
//!   used as the schema contract for downstream Redpanda consumers.
//! - **`.rs` files** — Rust structs with both `serde` XML derives (for
//!   decoding incoming WSDL messages) and `prost::Message` derives (for
//!   encoding outgoing protobuf messages). A single generated struct handles
//!   both sides of the normalization pipeline.
//!
//! ## Usage
//!
//! ```text
//! spear-gen --input <xsd-dir> [--out-proto <dir>] [--out-rust <dir>]
//! ```
//!
//! All `.xsd` files in `<xsd-dir>` are parsed in a single pass. Cross-file
//! type references are resolved automatically — no explicit import ordering
//! is required. Generated files are written to `generated/proto/` and
//! `generated/rust/` by default.
//!
//! ## Mapping rules
//!
//! See `docs/xsd-proto-mapping.md` in the workspace root for the full type
//! mapping table, handling of `xs:choice`, `xs:extension`, enumerations,
//! and known v1 limitations.

mod emitter;
mod mapping;
mod parser;

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;

/// Generate .proto files and Rust types from XSD schemas.
#[derive(Parser, Debug)]
#[command(name = "spear-gen", version)]
struct Args {
    /// Directory containing .xsd files (searched non-recursively).
    #[arg(short, long)]
    input: PathBuf,

    /// Output directory for generated .proto files.
    #[arg(long, default_value = "generated/proto")]
    out_proto: PathBuf,

    /// Output directory for generated Rust source files.
    #[arg(long, default_value = "generated/rust")]
    out_rust: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();

    eprintln!("spear-gen: reading XSD files from {}", args.input.display());

    let types = parser::load_schemas(&args.input)
        .context("failed to parse XSD schemas")?;

    eprintln!("spear-gen: found {} type definitions", types.len());

    std::fs::create_dir_all(&args.out_proto)
        .with_context(|| format!("creating {}", args.out_proto.display()))?;
    std::fs::create_dir_all(&args.out_rust)
        .with_context(|| format!("creating {}", args.out_rust.display()))?;

    // Emit one .proto file and one .rs file containing all types.
    // (Can be split per-source-file in a future pass if needed.)
    let proto_src = emitter::proto::emit(&types);
    let proto_path = args.out_proto.join("messages.proto");
    std::fs::write(&proto_path, &proto_src)
        .with_context(|| format!("writing {}", proto_path.display()))?;
    eprintln!("spear-gen: wrote {}", proto_path.display());

    let rust_src = emitter::rust::emit(&types);
    let rust_path = args.out_rust.join("messages.rs");
    std::fs::write(&rust_path, &rust_src)
        .with_context(|| format!("writing {}", rust_path.display()))?;
    eprintln!("spear-gen: wrote {}", rust_path.display());

    Ok(())
}
