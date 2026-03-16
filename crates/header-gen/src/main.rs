use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Parser;

use header_gen::config::{Endian, TargetConfig, WordSize};
use header_gen::{emitter, parser};

#[derive(Parser, Debug)]
#[command(
    name = "header-gen",
    about = "Generate Rust structs, proto3 messages, and mapping functions from C headers"
)]
struct Cli {
    /// Directory containing the C header files to process.
    #[arg(long)]
    input: PathBuf,

    /// Byte order of the binary data produced by the target system.
    #[arg(long, default_value = "little")]
    endian: EndianArg,

    /// Size of `long` and `unsigned long` on the target system.
    #[arg(long = "word-size", default_value = "32")]
    word_size: WordSizeArg,

    /// Pre-processor defines (repeatable, e.g. `--define LINUX --define UNIX`).
    #[arg(long = "define", value_name = "NAME")]
    defines: Vec<String>,

    /// Output directory for generated Rust structs (`structs.rs`,
    /// `review_report.txt`).
    #[arg(long = "out-rust", default_value = "generated/rust")]
    out_rust: PathBuf,

    /// Output directory for the generated proto file (`messages.proto`).
    #[arg(long = "out-proto", default_value = "generated/proto")]
    out_proto: PathBuf,

    /// Output directory for the generated mapping functions (`mappers.rs`).
    #[arg(long = "out-mapping", default_value = "generated/mapping")]
    out_mapping: PathBuf,
}

// ─── Clap value enums (thin wrappers around our domain types) ────────────────

#[derive(Debug, Clone, Copy)]
enum EndianArg {
    Little,
    Big,
}

impl std::str::FromStr for EndianArg {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "little" => Ok(EndianArg::Little),
            "big" => Ok(EndianArg::Big),
            _ => Err(format!("unknown endian `{s}`; expected `little` or `big`")),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum WordSizeArg {
    W32,
    W64,
}

impl std::str::FromStr for WordSizeArg {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "32" => Ok(WordSizeArg::W32),
            "64" => Ok(WordSizeArg::W64),
            _ => Err(format!("unknown word-size `{s}`; expected `32` or `64`")),
        }
    }
}

// ─── Entry point ─────────────────────────────────────────────────────────────

fn main() -> Result<()> {
    let cli = Cli::parse();

    let config = TargetConfig {
        endian: match cli.endian {
            EndianArg::Little => Endian::Little,
            EndianArg::Big => Endian::Big,
        },
        word_size: match cli.word_size {
            WordSizeArg::W32 => WordSize::W32,
            WordSizeArg::W64 => WordSize::W64,
        },
    };

    eprintln!(
        "header-gen: parsing `{}` (endian={}, word_size={})",
        cli.input.display(),
        config.endian,
        config.word_size,
    );

    // 1. Parse
    let (registry, report) = parser::parse(&cli.input, &cli.defines, config)
        .with_context(|| format!("Failed to parse headers in `{}`", cli.input.display()))?;

    eprintln!("  {} struct(s) discovered", registry.len());
    if !report.is_empty() {
        eprintln!(
            "  {} bitfield(s), {} union(s), {} unresolved, {} parse failure(s) — see review report",
            report.bitfields.len(),
            report.unions.len(),
            report.unresolved.len(),
            report.parse_failures.len(),
        );
    }

    // 2. Emit
    let proto_src = emitter::proto::emit(&registry, config);
    let rust_src = emitter::rust_structs::emit(&registry, config);
    let mapping_src = emitter::mapping::emit(&registry, config);
    let report_txt = report.render();

    // 3. Write outputs
    write_file(&cli.out_proto, "messages.proto", &proto_src)?;
    write_file(&cli.out_rust, "structs.rs", &rust_src)?;
    write_file(&cli.out_rust, "review_report.txt", &report_txt)?;
    write_file(&cli.out_mapping, "mappers.rs", &mapping_src)?;

    eprintln!("header-gen: done.");
    Ok(())
}

fn write_file(dir: &Path, filename: &str, content: &str) -> Result<()> {
    std::fs::create_dir_all(dir)
        .with_context(|| format!("Failed to create directory `{}`", dir.display()))?;
    let path = dir.join(filename);
    std::fs::write(&path, content)
        .with_context(|| format!("Failed to write `{}`", path.display()))?;
    eprintln!("  wrote {}", path.display());
    Ok(())
}
