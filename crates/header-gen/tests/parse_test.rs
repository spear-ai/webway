/// IR shape assertions — checks that the parser produces the right `CTypeRef`
/// values for different C constructs.
use std::path::PathBuf;

use header_gen::config::{Endian, TargetConfig, WordSize};
use header_gen::ir::{CPrimitive, CTypeRef};
use header_gen::parser;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

fn parse_single(filename: &str, config: TargetConfig) -> header_gen::ir::Registry {
    let dir = fixtures_dir().join(
        // Strip subdir if there is one, but keep simple names as-is.
        PathBuf::from(filename)
            .parent()
            .unwrap_or(std::path::Path::new(""))
            .to_path_buf(),
    );
    let base = PathBuf::from(filename)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    // For simple fixtures at the top level use the fixtures dir directly.
    let input_dir = if dir == PathBuf::from("") {
        fixtures_dir()
    } else {
        fixtures_dir().join(dir)
    };

    let (registry, _report) = parser::parse(&input_dir, &[], config)
        .unwrap_or_else(|e| panic!("parse failed for {base}: {e}"));
    registry
}

fn le32() -> TargetConfig {
    TargetConfig {
        endian: Endian::Little,
        word_size: WordSize::W32,
    }
}

fn le64() -> TargetConfig {
    TargetConfig {
        endian: Endian::Little,
        word_size: WordSize::W64,
    }
}

// ─── Word-size sensitivity ────────────────────────────────────────────────────

#[test]
fn long_is_i32_on_w32() {
    let reg = parse_single("simple.h", le32());
    let s = reg.get("SimpleScalars").expect("SimpleScalars not found");
    let field = s.fields.iter().find(|f| f.name == "a_long").unwrap();
    assert_eq!(field.ty, CTypeRef::Primitive(CPrimitive::Long));
    // Verify the mapping resolves to i32
    let m = header_gen::type_map::map_primitive(CPrimitive::Long, le32());
    assert_eq!(m.rust_type, "i32");
    assert_eq!(m.byte_size, 4);
}

#[test]
fn long_is_i64_on_w64() {
    let reg = parse_single("simple.h", le64());
    let s = reg.get("SimpleScalars").expect("SimpleScalars not found");
    let field = s.fields.iter().find(|f| f.name == "a_long").unwrap();
    assert_eq!(field.ty, CTypeRef::Primitive(CPrimitive::Long));
    let m = header_gen::type_map::map_primitive(CPrimitive::Long, le64());
    assert_eq!(m.rust_type, "i64");
    assert_eq!(m.byte_size, 8);
}

// ─── Arrays ───────────────────────────────────────────────────────────────────

#[test]
fn char_array_is_array_of_char() {
    let reg = parse_single("arrays.h", le32());
    let s = reg.get("Arrays").expect("Arrays not found");
    let field = s.fields.iter().find(|f| f.name == "name").unwrap();
    assert_eq!(
        field.ty,
        CTypeRef::Array(Box::new(CTypeRef::Primitive(CPrimitive::Char)), 32)
    );
}

#[test]
fn int_array_has_correct_count() {
    let reg = parse_single("arrays.h", le32());
    let s = reg.get("Arrays").expect("Arrays not found");
    let field = s.fields.iter().find(|f| f.name == "values").unwrap();
    assert_eq!(
        field.ty,
        CTypeRef::Array(Box::new(CTypeRef::Primitive(CPrimitive::Int)), 8)
    );
}

// ─── Typedef transparency ─────────────────────────────────────────────────────

#[test]
fn typedef_resolves_transparently() {
    let reg = parse_single("typedefs.h", le32());
    let s = reg.get("TypedefAlias").expect("TypedefAlias not found");
    let field = s.fields.iter().find(|f| f.name == "value").unwrap();
    // MyInt is typedef for int — should resolve to Int, not Unresolved
    assert_eq!(field.ty, CTypeRef::Primitive(CPrimitive::Int));
}

// ─── Bitfields ────────────────────────────────────────────────────────────────

#[test]
fn bitfield_width_is_recorded() {
    let reg = parse_single("bitfields.h", le32());
    let s = reg.get("Flags").expect("Flags not found");
    let field = s.fields.iter().find(|f| f.name == "active").unwrap();
    assert_eq!(field.bitfield_width, Some(1));
}

#[test]
fn bitfield_in_review_report() {
    let dir = fixtures_dir();
    let (_, report) = parser::parse(&dir, &[], le32()).unwrap();
    assert!(
        !report.bitfields.is_empty(),
        "expected bitfield items in report"
    );
    assert!(report
        .bitfields
        .iter()
        .any(|b| b.struct_name == "Flags" && b.field_name == "active"));
}

// ─── Unions ───────────────────────────────────────────────────────────────────

#[test]
fn union_becomes_raw_bytes() {
    let reg = parse_single("unions.h", le32());
    let s = reg.get("WithUnion").expect("WithUnion not found");
    let field = s.fields.iter().find(|f| f.name == "data").unwrap();
    assert!(
        matches!(field.ty, CTypeRef::Union { .. }),
        "expected Union variant, got {:?}",
        field.ty
    );
}

#[test]
fn union_in_review_report() {
    let dir = fixtures_dir();
    let (_, report) = parser::parse(&dir, &[], le32()).unwrap();
    assert!(!report.unions.is_empty(), "expected union items in report");
}

// ─── Nested struct ────────────────────────────────────────────────────────────

#[test]
fn nested_struct_field_is_struct_ref() {
    let reg = parse_single("nested.h", le32());
    let s = reg.get("Rect").expect("Rect not found");
    let field = s.fields.iter().find(|f| f.name == "top_left").unwrap();
    assert_eq!(field.ty, CTypeRef::Struct("Point".to_owned()));
}

// ─── Packed struct ────────────────────────────────────────────────────────────

#[test]
fn packed_struct_size_differs_from_aligned() {
    let reg = parse_single("packed.h", le32());
    let packed = reg.get("PackedRecord").expect("PackedRecord not found");
    let aligned = reg.get("AlignedRecord").expect("AlignedRecord not found");
    // PackedRecord: 1 + 4 + 2 = 7 bytes
    // AlignedRecord: 1 + 3(pad) + 4 + 2 + 2(pad) = 12 bytes (typical)
    assert_eq!(packed.total_byte_size, 7, "packed should be 7 bytes");
    assert!(
        aligned.total_byte_size > packed.total_byte_size,
        "aligned ({} bytes) should be larger than packed ({} bytes)",
        aligned.total_byte_size,
        packed.total_byte_size
    );
}
