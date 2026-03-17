/// Emitter output string assertions — checks that the three emitters produce
/// correct source code for known fixtures.
use std::path::PathBuf;

use header_gen::config::{Endian, TargetConfig, WordSize};
use header_gen::{emitter, parser};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

fn parse_all(config: TargetConfig) -> header_gen::ir::Registry {
    let (reg, _) = parser::parse(&fixtures_dir(), &[], &[], config).expect("parse failed");
    reg
}

fn le32() -> TargetConfig {
    TargetConfig {
        endian: Endian::Little,
        word_size: WordSize::W32,
    }
}

fn be32() -> TargetConfig {
    TargetConfig {
        endian: Endian::Big,
        word_size: WordSize::W32,
    }
}

fn le64() -> TargetConfig {
    TargetConfig {
        endian: Endian::Little,
        word_size: WordSize::W64,
    }
}

// ─── Rust struct emitter ──────────────────────────────────────────────────────

#[test]
fn rust_has_decode_fn() {
    let reg = parse_all(le32());
    let out = emitter::rust_structs::emit(&reg, le32());
    assert!(
        out.contains("pub fn decode"),
        "expected `pub fn decode` in:\n{out}"
    );
}

#[test]
fn rust_has_byte_size_fn() {
    let reg = parse_all(le32());
    let out = emitter::rust_structs::emit(&reg, le32());
    assert!(out.contains("pub fn byte_size"), "expected byte_size fn");
}

#[test]
fn rust_has_derive_attrs() {
    let reg = parse_all(le32());
    let out = emitter::rust_structs::emit(&reg, le32());
    assert!(out.contains("#[derive(Debug, Clone, PartialEq)]"));
}

#[test]
fn little_endian_uses_from_le_bytes() {
    let reg = parse_all(le32());
    let out = emitter::rust_structs::emit(&reg, le32());
    assert!(
        out.contains("from_le_bytes"),
        "expected from_le_bytes for little-endian"
    );
    assert!(
        !out.contains("from_be_bytes"),
        "should not have from_be_bytes for little-endian"
    );
}

#[test]
fn big_endian_uses_from_be_bytes() {
    let reg = parse_all(be32());
    let out = emitter::rust_structs::emit(&reg, be32());
    assert!(
        out.contains("from_be_bytes"),
        "expected from_be_bytes for big-endian"
    );
    assert!(
        !out.contains("from_le_bytes"),
        "should not have from_le_bytes for big-endian"
    );
}

#[test]
fn long_field_is_i32_on_w32() {
    let reg = parse_all(le32());
    let out = emitter::rust_structs::emit(&reg, le32());
    // SimpleScalars::a_long should be i32 with W32
    assert!(
        out.contains("pub a_long: i32"),
        "expected `pub a_long: i32` in:\n{out}"
    );
}

#[test]
fn long_field_is_i64_on_w64() {
    let reg = parse_all(le64());
    let out = emitter::rust_structs::emit(&reg, le64());
    assert!(
        out.contains("pub a_long: i64"),
        "expected `pub a_long: i64` in:\n{out}"
    );
}

#[test]
fn char_array_field_is_u8_array() {
    let reg = parse_all(le32());
    let out = emitter::rust_structs::emit(&reg, le32());
    // name: [u8; 32]
    assert!(
        out.contains("[u8; 32]"),
        "expected char[32] → [u8; 32] in:\n{out}"
    );
}

#[test]
fn bitfield_omitted_from_rust() {
    let reg = parse_all(le32());
    let out = emitter::rust_structs::emit(&reg, le32());
    // Flags struct bitfields should be replaced with a comment
    assert!(
        out.contains("bitfield") && out.contains("omitted"),
        "expected bitfield omission comment"
    );
}

#[test]
fn union_field_is_raw_bytes_in_rust() {
    let reg = parse_all(le32());
    let out = emitter::rust_structs::emit(&reg, le32());
    // WithUnion::data should become [u8; 4]
    assert!(
        out.contains("pub data: [u8;"),
        "expected union field as [u8; N] in:\n{out}"
    );
}

// ─── Offset correctness ───────────────────────────────────────────────────────

#[test]
fn padded_struct_uses_libclang_offsets() {
    // AlignedRecord (packed.h): char tag (offset 0), int value (offset 4), short flags (offset 8).
    // Without libclang offsets the emitter would wrongly place `value` at offset 1.
    let reg = parse_all(le32());
    let out = emitter::rust_structs::emit(&reg, le32());

    // Extract just the AlignedRecord impl block.
    let start = out
        .find("impl AlignedRecord")
        .expect("AlignedRecord not found in output");
    let after_start = &out[start..];
    // The block ends at the closing `}` before the next top-level item.
    let end = after_start
        .find("\n}\n")
        .map(|i| i + 3)
        .unwrap_or(after_start.len());
    let block = &after_start[..end];

    // `tag` is at byte 0.
    assert!(
        block.contains("_ofs = 0;"),
        "expected `_ofs = 0` for char tag:\n{block}"
    );
    // `value` (int) must be at byte 4, not byte 1.
    assert!(
        block.contains("_ofs = 4;"),
        "expected `_ofs = 4` for int value (padding after char):\n{block}"
    );
    assert!(
        !block.contains("_ofs = 1;"),
        "found incorrect `_ofs = 1` — padding not accounted for:\n{block}"
    );
}

// ─── Proto emitter ────────────────────────────────────────────────────────────

#[test]
fn proto_has_syntax_header() {
    let reg = parse_all(le32());
    let out = emitter::proto::emit(&reg, le32());
    assert!(out.starts_with("syntax = \"proto3\";"));
}

#[test]
fn proto_char_array_is_bytes() {
    let reg = parse_all(le32());
    let out = emitter::proto::emit(&reg, le32());
    // name[32] should become `bytes name = 1;`
    assert!(
        out.contains("bytes name"),
        "expected `bytes name` in proto output:\n{out}"
    );
}

#[test]
fn proto_has_message_for_each_struct() {
    let reg = parse_all(le32());
    let out = emitter::proto::emit(&reg, le32());
    for name in reg.keys() {
        assert!(
            out.contains(&format!("message {name}")),
            "missing message for {name}"
        );
    }
}

#[test]
fn proto_bitfields_omitted_with_comment() {
    let reg = parse_all(le32());
    let out = emitter::proto::emit(&reg, le32());
    assert!(
        out.contains("bitfields omitted"),
        "expected bitfields omitted comment in proto"
    );
}

// ─── Mapping emitter ──────────────────────────────────────────────────────────

#[test]
fn mapping_has_to_vec_for_char_array() {
    let reg = parse_all(le32());
    let out = emitter::mapping::emit(&reg, le32());
    assert!(
        out.contains("to_vec()"),
        "expected .to_vec() for char array in mapping:\n{out}"
    );
}

#[test]
fn mapping_has_cast_for_small_types() {
    let reg = parse_all(le32());
    let out = emitter::mapping::emit(&reg, le32());
    // char → i32 needs `as i32`
    assert!(
        out.contains("as i32") || out.contains("as u32"),
        "expected `as i32` or `as u32` cast for char/short fields"
    );
}

#[test]
fn mapping_has_map_fn_for_each_struct() {
    let reg = parse_all(le32());
    let out = emitter::mapping::emit(&reg, le32());
    for name in reg.keys() {
        let snake: String = {
            let mut s = String::new();
            let chars: Vec<char> = name.chars().collect();
            for (i, &c) in chars.iter().enumerate() {
                if c.is_uppercase() && i > 0 {
                    s.push('_');
                }
                s.push(c.to_lowercase().next().unwrap());
            }
            s
        };
        assert!(
            out.contains(&format!("fn map_{snake}")),
            "missing map_{snake} function in mapping output"
        );
    }
}
