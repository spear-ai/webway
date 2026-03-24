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
    let (reg, report) =
        parser::parse(&fixtures_dir(), &[], &[], le32(), false).expect("parse failed");
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

    let (_, report) = parser::parse(tmp.path(), &[], &[], le32(), false).expect("parse failed");
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

    let (_, report) = parser::parse(tmp.path(), &[], &[], le32(), false).expect("parse failed");
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

    let (_, report) = parser::parse(tmp.path(), &[], &[], le32(), false).expect("parse failed");
    assert!(
        !report.unions.is_empty(),
        "expected union items in review report"
    );
}

// ─── Big-endian 64-bit target ─────────────────────────────────────────────────

#[test]
fn big_endian_64_outputs_correct_markers() {
    let (reg, _) = parser::parse(&fixtures_dir(), &[], &[], be64(), false).expect("parse failed");
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

// ─── Include path filtering ───────────────────────────────────────────────────

/// Verifies that structs defined in --include dirs are NOT emitted — only
/// structs in --input headers appear in the registry. This mirrors the
/// real-world setup: headers/ (input), includes/ (system), lm-includes/ (LM).
#[test]
fn structs_from_include_dirs_are_not_emitted() {
    let input_dir = tempfile::tempdir().expect("input tempdir");
    let include_dir = tempfile::tempdir().expect("include tempdir");

    // External type in the include dir — should NOT appear in output.
    std::fs::write(
        include_dir.path().join("ext_types.h"),
        "typedef struct ExtType { int x; } ExtType;\n",
    )
    .unwrap();

    // User header in the input dir — SHOULD appear in output.
    // It references ExtType from the include dir.
    std::fs::write(
        input_dir.path().join("user.h"),
        "#include <ext_types.h>\ntypedef struct UserRecord { int id; ExtType ext; } UserRecord;\n",
    )
    .unwrap();

    let include_flag = include_dir.path().to_string_lossy().into_owned();
    let (reg, _) =
        parser::parse(input_dir.path(), &[include_flag], &[], le32(), false).expect("parse failed");

    assert!(
        reg.contains_key("UserRecord"),
        "UserRecord (from input dir) should be in registry"
    );
    assert!(
        !reg.contains_key("ExtType"),
        "ExtType (from include dir) must not be in registry — only input headers are emitted"
    );
}

// ─── Typedef struct patterns ──────────────────────────────────────────────────

/// The two most common C struct patterns both set key == typedef_name in the
/// TypedefDecl handler.  A prior bug inserted typedef_name into `seen` before
/// checking `!seen.contains(&key)`, causing BOTH patterns to silently emit 0
/// structs even though the file passed the input-dir filter.
#[test]
fn typedef_struct_same_name_is_registered() {
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        tmp.path().join("common.h"),
        // Pattern 1: typedef struct Foo { ... } Foo;   (named, matching)
        "typedef struct Packet { int id; int len; } Packet;\n\
         // Pattern 2: typedef struct { ... } Bar;      (anonymous)
         typedef struct { int x; int y; } Point;\n",
    )
    .unwrap();

    let (reg, report) = parser::parse(tmp.path(), &[], &[], le32(), false).expect("parse failed");
    assert!(
        report.parse_failures.is_empty(),
        "parse failures: {:?}",
        report.parse_failures
    );
    assert!(
        reg.contains_key("Packet"),
        "Packet (named typedef struct) missing; keys: {:?}",
        reg.keys().collect::<Vec<_>>()
    );
    assert!(
        reg.contains_key("Point"),
        "Point (anonymous typedef struct) missing; keys: {:?}",
        reg.keys().collect::<Vec<_>>()
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

    let (_, report) = parser::parse(tmp.path(), &[], &[], le32(), false).expect("parse failed");
    assert!(
        report.unresolved.is_empty(),
        "unexpected unresolved types: {:?}",
        report.unresolved
    );
}

// ─── Realistic filtering tests ────────────────────────────────────────────────
// These cover the failure modes seen in the container that the basic tempdir
// test misses. Each test documents a specific scenario in the pattern:
//   write inline header → parse → assert registry contents
//
// New container-specific scenarios should be added here following this pattern.

/// Verifies that internal types from system headers (stdint.h etc.) do not
/// bleed into the registry when a user header #includes them.
///
/// This is the core failure mode from v0.2.18-v0.2.20: system types like
/// __pthread_internal_list were appearing in output instead of user structs.
/// Skipped at runtime if system headers are not found — avoids CI failures
/// on unusual environments while still running on macOS + ubuntu-latest.
#[test]
fn system_types_do_not_bleed_through_include() {
    use std::path::PathBuf;

    // Probe for system stdint.h — macOS SDK first, then Linux /usr/include.
    let sdk_include = std::process::Command::new("xcrun")
        .args(["--show-sdk-path"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| {
            PathBuf::from(String::from_utf8_lossy(&o.stdout).trim().to_string()).join("usr/include")
        });

    let system_include = sdk_include
        .filter(|p| p.join("stdint.h").exists())
        .or_else(|| {
            let linux = PathBuf::from("/usr/include");
            linux.join("stdint.h").exists().then_some(linux)
        });

    let system_include = match system_include {
        Some(p) => p,
        None => {
            eprintln!("system_types_do_not_bleed_through_include: skipped (stdint.h not found)");
            return;
        }
    };

    let input_dir = tempfile::tempdir().expect("input tempdir");
    std::fs::write(
        input_dir.path().join("mission.h"),
        "#include <stdint.h>\ntypedef struct MissionStatus { uint32_t id; uint32_t flags; } MissionStatus;\n",
    ).unwrap();

    let include_flag = system_include.to_string_lossy().into_owned();
    let (reg, report) =
        parser::parse(input_dir.path(), &[include_flag], &[], le32(), false).expect("parse failed");

    // Only fail on parse errors from the user's input files — system headers may
    // have missing 32-bit stubs or multiarch fragments depending on the environment.
    let input_failures: Vec<_> = report
        .parse_failures
        .iter()
        .filter(|f| f.file.starts_with(input_dir.path().to_str().unwrap_or("")))
        .collect();
    assert!(
        input_failures.is_empty(),
        "unexpected parse failures in input files: {:?}",
        input_failures
    );
    assert!(
        reg.contains_key("MissionStatus"),
        "MissionStatus (from input dir) must be in registry; found: {:?}",
        reg.keys().collect::<Vec<_>>()
    );
    // glibc/Darwin internal structs always start with _ — none should leak.
    let leaked: Vec<&String> = reg.keys().filter(|k| k.starts_with('_')).collect();
    assert!(
        leaked.is_empty(),
        "system/internal types leaked into registry: {:?}",
        leaked
    );
}

/// Verifies that the filter works when --input is a symlink to the real
/// header directory — simulating container bind mounts where the user-visible
/// path differs from the canonical path libclang resolves internally.
#[test]
#[cfg(unix)]
fn filter_works_through_symlinked_input_dir() {
    let real_dir = tempfile::tempdir().expect("real tempdir");
    std::fs::write(
        real_dir.path().join("target.h"),
        "typedef struct TargetStruct { int x; int y; } TargetStruct;\n",
    )
    .unwrap();

    let link_path = real_dir
        .path()
        .parent()
        .unwrap()
        .join("hg_test_linked_headers");
    let _ = std::fs::remove_file(&link_path); // clean up any leftover from prior run
    std::os::unix::fs::symlink(real_dir.path(), &link_path).expect("failed to create symlink");

    let result = parser::parse(&link_path, &[], &[], le32(), false);
    let _ = std::fs::remove_file(&link_path); // always clean up

    let (reg, report) = result.expect("parse failed");

    assert!(
        report.parse_failures.is_empty(),
        "parse failures: {:?}",
        report.parse_failures
    );
    assert!(
        reg.contains_key("TargetStruct"),
        "TargetStruct missing when input_dir is a symlink; \
         keys: {:?}\nHint: libclang resolved through the symlink but \
         canonical_input_dir was computed differently.",
        reg.keys().collect::<Vec<_>>()
    );
}

/// Verifies that the filter works when input_dir contains `..` components
/// (i.e., is not in canonical form). Reproduces the case where a user runs
/// from a directory with a non-canonical $PWD or constructs a path manually.
#[test]
fn filter_works_with_dotdot_in_input_dir() {
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        tmp.path().join("widget.h"),
        "typedef struct Widget { int id; float value; } Widget;\n",
    )
    .unwrap();

    // Build a valid but non-canonical path: /tmp/tmpXYZ/../tmpXYZ
    let wobbly = tmp
        .path()
        .join("..")
        .join(tmp.path().file_name().expect("tempdir has a file name"));

    let (reg, report) = parser::parse(&wobbly, &[], &[], le32(), false).expect("parse failed");

    assert!(
        report.parse_failures.is_empty(),
        "parse failures: {:?}",
        report.parse_failures
    );
    assert!(
        reg.contains_key("Widget"),
        "Widget missing when input_dir has .. components; \
         input=`{}`, canonical=`{}`\nkeys: {:?}",
        wobbly.display(),
        wobbly.canonicalize().unwrap_or_default().display(),
        reg.keys().collect::<Vec<_>>()
    );
}
