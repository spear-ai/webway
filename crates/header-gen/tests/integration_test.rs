/// End-to-end integration tests: parse fixture headers and verify all three
/// outputs (Rust structs, proto, mapping) are generated correctly.
use std::path::PathBuf;

use header_gen::config::{Endian, TargetConfig, WordSize};
use header_gen::{emitter, parser};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

fn le32() -> TargetConfig {
    TargetConfig {
        endian: Endian::Little,
        word_size: WordSize::W32,
    }
}

fn be64() -> TargetConfig {
    TargetConfig {
        endian: Endian::Big,
        word_size: WordSize::W64,
    }
}

// ─── All-three-outputs for simple.h ─────────────────────────────────────────

#[test]
fn all_three_outputs_generated_for_simple_h() {
    let (reg, report) = parser::parse(&fixtures_dir(), &[], &[], le32()).expect("parse failed");
    assert!(!reg.is_empty(), "expected at least one struct");

    let rust_out = emitter::rust_structs::emit(&reg, le32());
    let proto_out = emitter::proto::emit(&reg, le32());
    let map_out = emitter::mapping::emit(&reg, le32());

    // All outputs must be non-empty.
    assert!(!rust_out.is_empty());
    assert!(!proto_out.is_empty());
    assert!(!map_out.is_empty());

    // SimpleScalars must appear in all three.
    assert!(
        rust_out.contains("SimpleScalars"),
        "rust output missing SimpleScalars"
    );
    assert!(
        proto_out.contains("SimpleScalars"),
        "proto output missing SimpleScalars"
    );
    assert!(
        map_out.contains("simple_scalars") || map_out.contains("SimpleScalars"),
        "mapping output missing simple_scalars"
    );

    // Report may have items (bitfields.h, unions.h are in the fixture dir)
    // but should not contain parse failures.
    assert!(
        report.parse_failures.is_empty(),
        "unexpected parse failures: {:?}",
        report.parse_failures
    );
}

// ─── Review report: clean for simple header ──────────────────────────────────

#[test]
fn review_report_empty_for_simple_scalars_only() {
    // Parse only simple.h by creating a temp dir with only that file.
    let tmp = tempfile::tempdir().expect("tempdir");
    let src = fixtures_dir().join("simple.h");
    std::fs::copy(&src, tmp.path().join("simple.h")).unwrap();

    let (_, report) = parser::parse(tmp.path(), &[], &[], le32()).expect("parse failed");
    assert!(
        report.is_empty(),
        "expected empty review report for simple.h, got:\n{}",
        report.render()
    );
}

// ─── Review report: items for bitfield/union headers ─────────────────────────

#[test]
fn review_report_has_items_for_bitfield_headers() {
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::copy(
        fixtures_dir().join("bitfields.h"),
        tmp.path().join("bitfields.h"),
    )
    .unwrap();

    let (_, report) = parser::parse(tmp.path(), &[], &[], le32()).expect("parse failed");
    assert!(
        !report.bitfields.is_empty(),
        "expected bitfield items in review report"
    );
    let rendered = report.render();
    assert!(
        rendered.contains("BITFIELDS"),
        "rendered report missing BITFIELDS section"
    );
}

#[test]
fn review_report_has_union_items() {
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::copy(fixtures_dir().join("unions.h"), tmp.path().join("unions.h")).unwrap();

    let (_, report) = parser::parse(tmp.path(), &[], &[], le32()).expect("parse failed");
    assert!(
        !report.unions.is_empty(),
        "expected union items in review report"
    );
}

// ─── Big-endian 64-bit target ─────────────────────────────────────────────────

#[test]
fn big_endian_64_outputs_correct_markers() {
    let (reg, _) = parser::parse(&fixtures_dir(), &[], &[], be64()).expect("parse failed");
    let rust_out = emitter::rust_structs::emit(&reg, be64());
    let proto_out = emitter::proto::emit(&reg, be64());

    assert!(
        rust_out.contains("from_be_bytes"),
        "expected from_be_bytes in BE output"
    );
    assert!(
        rust_out.contains("pub a_long: i64"),
        "expected i64 long on W64"
    );
    assert!(
        proto_out.contains("endian=big"),
        "expected endian=big in proto header"
    );
}

// ─── Typedef resolution ───────────────────────────────────────────────────────

#[test]
fn typedef_aliases_produce_no_unresolved_types() {
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::copy(
        fixtures_dir().join("typedefs.h"),
        tmp.path().join("typedefs.h"),
    )
    .unwrap();

    let (_, report) = parser::parse(tmp.path(), &[], &[], le32()).expect("parse failed");
    assert!(
        report.unresolved.is_empty(),
        "unexpected unresolved types: {:?}",
        report.unresolved
    );
}
