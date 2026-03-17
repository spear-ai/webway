use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;

use anyhow::{anyhow, Result};
use clang::{Clang, EntityKind, Index, TypeKind, Unsaved};

// libclang is not thread-safe and only allows one `Clang` instance at a time.
// This mutex serializes all calls into libclang so the test suite (which runs
// tests in parallel by default) does not fail with "an instance already exists".
static CLANG_LOCK: Mutex<()> = Mutex::new(());

use crate::config::TargetConfig;
use crate::ir::{CField, CPrimitive, CStruct, CTypeRef, Registry};
use crate::report::{BitfieldItem, ParseFailure, ReviewReport, UnionItem, UnresolvedItem};

/// Parse all `headers` as a single translation unit (via an in-memory umbrella
/// file) and return the discovered struct registry plus a review report.
pub fn parse_headers(
    headers: &[PathBuf],
    include_flags: &[String],
    defines: &[String],
    config: TargetConfig,
) -> Result<(Registry, ReviewReport)> {
    // Hold the global lock for the entire duration of this parse so that
    // concurrent test threads don't try to create multiple Clang instances.
    let _lock = CLANG_LOCK
        .lock()
        .map_err(|_| anyhow!("CLANG_LOCK poisoned"))?;

    let clang = Clang::new().map_err(|e| anyhow!("Failed to initialise libclang: {e}"))?;
    let index = Index::new(&clang, false, false);

    // Build an umbrella source that includes every discovered header.
    let umbrella_src: String = headers
        .iter()
        .map(|h| format!("#include \"{}\"\n", h.display()))
        .collect();

    let args = config.clang_flags(include_flags, defines);

    let tu = index
        .parser("umbrella.h")
        .arguments(&args)
        .unsaved(&[Unsaved::new("umbrella.h", &umbrella_src)])
        .parse()
        .map_err(|e| anyhow!("libclang parse error: {e:?}"))?;

    // Collect parse diagnostics.
    let mut report = ReviewReport::default();
    for diag in tu.get_diagnostics() {
        use clang::diagnostic::Severity;
        if diag.get_severity() >= Severity::Error {
            let loc = diag.get_location().get_file_location();
            let file = loc
                .file
                .map(|f| f.get_path().display().to_string())
                .unwrap_or_else(|| "<unknown>".to_owned());
            report.parse_failures.push(ParseFailure {
                file,
                message: diag.get_text(),
            });
        }
    }

    let mut registry = Registry::new();
    let mut seen: HashSet<String> = HashSet::new();

    // Only register structs whose definition lives in one of the user's input
    // headers. Without this filter, every struct from transitively-included
    // system headers (glibc internals, pthread types, etc.) would also appear
    // in the output.
    let input_files: HashSet<PathBuf> = headers.iter().cloned().collect();

    let in_input_headers = |cursor: &clang::Entity| -> bool {
        cursor
            .get_location()
            .and_then(|loc| loc.get_file_location().file)
            .map(|f| {
                let p = f.get_path();
                input_files.contains(&p)
                    || p.canonicalize()
                        .map(|c| {
                            input_files
                                .iter()
                                .any(|h| h.canonicalize().ok() == Some(c.clone()))
                        })
                        .unwrap_or(false)
            })
            .unwrap_or(false)
    };

    tu.get_entity().visit_children(|cursor, _parent| {
        match cursor.get_kind() {
            EntityKind::StructDecl => {
                if !in_input_headers(&cursor) {
                    return clang::EntityVisitResult::Continue;
                }
                if let Some(name) = cursor.get_name() {
                    if !name.is_empty() && !seen.contains(&name) {
                        seen.insert(name.clone());
                        if let Some(s) = visit_struct(&cursor, &name, &mut report) {
                            registry.insert(name, s);
                        }
                    }
                }
                clang::EntityVisitResult::Continue
            }
            EntityKind::TypedefDecl => {
                if !in_input_headers(&cursor) {
                    return clang::EntityVisitResult::Continue;
                }
                // Handle `typedef struct Foo { ... } Foo;`
                if let Some(ty) = cursor.get_typedef_underlying_type() {
                    let canon = ty.get_canonical_type();
                    if canon.get_kind() == TypeKind::Record {
                        if let Some(decl) = canon.get_declaration() {
                            if decl.get_kind() == EntityKind::StructDecl {
                                // Use the typedef name as the canonical name
                                // if the struct itself is anonymous.
                                let typedef_name = cursor.get_name().unwrap_or_default();
                                let struct_name =
                                    decl.get_name().unwrap_or_else(|| typedef_name.clone());
                                let key = if struct_name.is_empty() {
                                    typedef_name.clone()
                                } else {
                                    struct_name.clone()
                                };
                                // Register under the typedef name too so
                                // field references resolve.
                                if !seen.contains(&typedef_name) {
                                    seen.insert(typedef_name.clone());
                                    if !seen.contains(&key) {
                                        seen.insert(key.clone());
                                        if let Some(s) = visit_struct(&decl, &key, &mut report) {
                                            registry.insert(key.clone(), s.clone());
                                        }
                                    }
                                    // If typedef name differs from struct name,
                                    // insert an alias entry.
                                    if typedef_name != key {
                                        if let Some(s) = registry.get(&key).cloned() {
                                            registry.insert(typedef_name, s);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                clang::EntityVisitResult::Continue
            }
            _ => clang::EntityVisitResult::Continue,
        }
    });

    Ok((registry, report))
}

fn visit_struct(cursor: &clang::Entity, name: &str, report: &mut ReviewReport) -> Option<CStruct> {
    let ty = cursor.get_type()?;
    let total_byte_size = ty.get_sizeof().ok()? as u64;

    let mut fields = Vec::new();

    cursor.visit_children(|child, _| {
        if child.get_kind() != EntityKind::FieldDecl {
            return clang::EntityVisitResult::Continue;
        }

        let field_name = child.get_name().unwrap_or_else(|| "_unnamed".to_owned());
        let bitfield_width = child.get_bit_field_width().map(|w| w as u32);

        if let Some(width) = bitfield_width {
            report.bitfields.push(BitfieldItem {
                struct_name: name.to_owned(),
                field_name: field_name.clone(),
                width,
            });
        }

        let field_ty = match child.get_type() {
            Some(t) => map_type(t, name, &field_name, report),
            None => CTypeRef::Unresolved("<no type>".to_owned()),
        };

        // get_offset_of_field() returns the offset in bits (usize); divide by 8 for bytes.
        // Falls back to 0 on error (a 0-offset field is always correct for the first field).
        let byte_offset = child
            .get_offset_of_field()
            .map(|bits| (bits / 8) as u64)
            .unwrap_or(0);

        fields.push(CField {
            name: field_name,
            ty: field_ty,
            byte_offset,
            bitfield_width,
        });

        clang::EntityVisitResult::Continue
    });

    Some(CStruct {
        name: name.to_owned(),
        fields,
        total_byte_size,
    })
}

fn map_type(
    ty: clang::Type,
    struct_name: &str,
    field_name: &str,
    report: &mut ReviewReport,
) -> CTypeRef {
    match ty.get_kind() {
        TypeKind::CharS | TypeKind::SChar => CTypeRef::Primitive(CPrimitive::Char),
        TypeKind::CharU | TypeKind::UChar => CTypeRef::Primitive(CPrimitive::UChar),
        TypeKind::Short => CTypeRef::Primitive(CPrimitive::Short),
        TypeKind::UShort => CTypeRef::Primitive(CPrimitive::UShort),
        TypeKind::Int => CTypeRef::Primitive(CPrimitive::Int),
        TypeKind::UInt => CTypeRef::Primitive(CPrimitive::UInt),
        TypeKind::Long => CTypeRef::Primitive(CPrimitive::Long),
        TypeKind::ULong => CTypeRef::Primitive(CPrimitive::ULong),
        TypeKind::LongLong => CTypeRef::Primitive(CPrimitive::LongLong),
        TypeKind::ULongLong => CTypeRef::Primitive(CPrimitive::ULongLong),
        TypeKind::Float => CTypeRef::Primitive(CPrimitive::Float),
        TypeKind::Double => CTypeRef::Primitive(CPrimitive::Double),

        TypeKind::ConstantArray => {
            let count = ty.get_size().unwrap_or(0) as u64;
            let elem_ty = ty
                .get_element_type()
                .map(|et| map_type(et, struct_name, field_name, report))
                .unwrap_or(CTypeRef::Unresolved("<unknown elem>".to_owned()));
            CTypeRef::Array(Box::new(elem_ty), count)
        }

        TypeKind::Record => {
            let decl = ty.get_declaration();
            let is_union = decl
                .as_ref()
                .map(|d| d.get_kind() == EntityKind::UnionDecl)
                .unwrap_or(false);

            if is_union {
                let byte_size = ty.get_sizeof().unwrap_or(0) as u64;
                report.unions.push(UnionItem {
                    struct_name: struct_name.to_owned(),
                    field_name: field_name.to_owned(),
                    byte_size,
                });
                CTypeRef::Union { byte_size }
            } else {
                let sname = decl
                    .and_then(|d| d.get_name())
                    .unwrap_or_else(|| ty.get_display_name());
                CTypeRef::Struct(sname)
            }
        }

        // Transparent: resolve through typedef/elaborated type.
        TypeKind::Typedef | TypeKind::Elaborated => {
            let canon = ty.get_canonical_type();
            map_type(canon, struct_name, field_name, report)
        }

        _ => {
            let name_str = ty.get_display_name();
            report.unresolved.push(UnresolvedItem {
                struct_name: struct_name.to_owned(),
                field_name: field_name.to_owned(),
                type_name: name_str.clone(),
            });
            CTypeRef::Unresolved(name_str)
        }
    }
}
