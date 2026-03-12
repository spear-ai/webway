//! XSD parser — reads `.xsd` files from a directory and produces an
//! [`ast::TypeDef`] list that emitters consume.
//!
//! Parsing is two-pass:
//! 1. Every file is parsed into raw [`ast::TypeDef`] values and inserted
//!    into a name index. Primitive type aliases (`simpleType` with a
//!    primitive restriction base) are collected separately.
//! 2. [`resolve_extensions`] walks the index and flattens `xs:extension`
//!    base-type fields into child types so emitters never see
//!    [`ComplexContent::Extension`] in practice.
//! 3. [`apply_type_aliases`] substitutes `TypeRef::Named(alias)` →
//!    `TypeRef::Builtin(p)` for every known primitive alias.
//!
//! Cross-file references (a type in `alert.xsd` referencing an enum from
//! `track.xsd`) are handled naturally because all files are loaded before
//! any resolution step runs.
//!
//! Directory scanning is recursive: every `.xsd` file in any subdirectory
//! under `dir` is included.

pub mod ast;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use roxmltree::Node;

use ast::*;

const XS: &str = "http://www.w3.org/2001/XMLSchema";

/// Load all XSD files in `dir` (recursively), resolve cross-file type
/// references, and return a flat list of all type definitions.
pub fn load_schemas(dir: &Path) -> Result<Vec<TypeDef>> {
    let xsd_files = collect_xsd_files(dir)?;

    // First pass: parse every file into raw TypeDefs and collect primitive
    // type aliases (simpleType with a non-enum restriction base).
    let mut all_types: Vec<TypeDef> = Vec::new();
    let mut type_index: HashMap<String, usize> = HashMap::new();
    // Direct aliases: name → Primitive (e.g. CToken → Bytes)
    let mut type_aliases: HashMap<String, Primitive> = HashMap::new();
    // Pending: name → base type name not yet resolved to a primitive.
    // Used to resolve chained aliases (A restricts B which restricts xs:base64Binary).
    let mut pending_aliases: HashMap<String, String> = HashMap::new();

    for path in &xsd_files {
        let src =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let doc = roxmltree::Document::parse(&src)
            .with_context(|| format!("parsing {}", path.display()))?;

        // Collect primitive aliases from this file before parsing TypeDefs.
        collect_type_aliases(&doc, &mut type_aliases, &mut pending_aliases);

        let defs = parse_document(&doc)
            .with_context(|| format!("extracting types from {}", path.display()))?;

        for def in defs {
            let name = def.name().to_owned();
            if type_index.contains_key(&name) {
                eprintln!(
                    "warning: duplicate type '{}' in {}; keeping first definition",
                    name,
                    path.display()
                );
                continue;
            }
            let idx = all_types.len();
            all_types.push(def);
            type_index.insert(name, idx);
        }
    }

    // Resolve chained aliases iteratively:
    // e.g. A restricts B, B restricts xs:base64Binary → both become Bytes.
    loop {
        let mut resolved_any = false;
        pending_aliases.retain(|name, base| {
            if let Some(&p) = type_aliases.get(base.as_str()) {
                type_aliases.insert(name.clone(), p);
                resolved_any = true;
                false // resolved — remove from pending
            } else if let TypeRef::Builtin(p) = resolve_type_ref(base) {
                type_aliases.insert(name.clone(), p);
                resolved_any = true;
                false
            } else {
                true // still unresolved — keep
            }
        });
        if !resolved_any {
            break;
        }
    }

    eprintln!(
        "spear-gen: resolved {} primitive type alias(es)",
        type_aliases.len()
    );

    // Second pass: resolve xs:extension base names.
    let resolved = resolve_extensions(all_types, &type_index)?;

    // Third pass: substitute primitive aliases so emitters never see
    // unresolved TypeRef::Named for names like CUserSecurityToken.
    let resolved = apply_type_aliases(resolved, &type_aliases);

    Ok(resolved)
}

/// Recursively collect all `.xsd` files under `dir`, sorted for determinism.
/// Uses `entry.file_type()` (does not follow symlinks) to avoid infinite
/// loops if the directory tree contains circular symlinks.
fn collect_xsd_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    let mut stack = vec![dir.to_path_buf()];

    while let Some(current) = stack.pop() {
        for entry in std::fs::read_dir(&current)
            .with_context(|| format!("reading directory {}", current.display()))?
        {
            let entry = entry?;
            let ft = entry
                .file_type()
                .with_context(|| format!("stat {}", entry.path().display()))?;
            if ft.is_dir() {
                stack.push(entry.path());
            } else if ft.is_file()
                && entry
                    .path()
                    .extension()
                    .and_then(|e| e.to_str())
                    == Some("xsd")
            {
                paths.push(entry.path());
            }
            // symlinks are intentionally skipped
        }
    }

    paths.sort();
    Ok(paths)
}

/// Scan a document for `simpleType` elements whose `xs:restriction` base has
/// no `xs:enumeration` children.  Direct primitive restrictions go straight
/// into `aliases`; restrictions whose base is another named type go into
/// `pending` for transitive resolution after all files are loaded.
fn collect_type_aliases(
    doc: &roxmltree::Document,
    aliases: &mut HashMap<String, Primitive>,
    pending: &mut HashMap<String, String>,
) {
    let root = doc.root_element();
    for child in root.children().filter(|n| n.is_element()) {
        if child.tag_name().name() != "simpleType" {
            continue;
        }
        let name = match child.attribute("name") {
            Some(n) => n.trim(),
            None => continue,
        };
        if name.is_empty() {
            continue;
        }
        let restriction = match find_child_ns(child, XS, "restriction") {
            Some(r) => r,
            None => continue,
        };
        // Only treat as an alias if there are no enumeration children.
        let has_enums = restriction
            .children()
            .any(|n| n.is_element() && n.tag_name().name() == "enumeration");
        if has_enums {
            continue;
        }
        let base = match restriction.attribute("base") {
            Some(b) => b.trim(),
            None => continue,
        };
        match resolve_type_ref(base) {
            TypeRef::Builtin(p) => {
                aliases.insert(name.to_owned(), p);
            }
            TypeRef::Named(named_base) => {
                // Chain: this alias's base is itself a named type.
                // Record it for transitive resolution.
                pending.insert(name.to_owned(), named_base);
            }
        }
    }
}

/// Walk all TypeDef fields and replace `TypeRef::Named(alias)` with
/// `TypeRef::Builtin(p)` for every alias in the map.
fn apply_type_aliases(types: Vec<TypeDef>, aliases: &HashMap<String, Primitive>) -> Vec<TypeDef> {
    if aliases.is_empty() {
        return types;
    }
    types
        .into_iter()
        .map(|def| match def {
            TypeDef::Complex(ct) => TypeDef::Complex(ComplexType {
                name: ct.name,
                content: map_content_aliases(ct.content, aliases),
            }),
            other => other,
        })
        .collect()
}

fn map_content_aliases(
    content: ComplexContent,
    aliases: &HashMap<String, Primitive>,
) -> ComplexContent {
    match content {
        ComplexContent::Sequence(fields) => {
            ComplexContent::Sequence(map_fields_aliases(fields, aliases))
        }
        ComplexContent::Choice(fields) => {
            ComplexContent::Choice(map_fields_aliases(fields, aliases))
        }
        ComplexContent::Extension { base, extra_fields } => ComplexContent::Extension {
            base,
            extra_fields: map_fields_aliases(extra_fields, aliases),
        },
    }
}

fn map_fields_aliases(fields: Vec<Field>, aliases: &HashMap<String, Primitive>) -> Vec<Field> {
    fields
        .into_iter()
        .map(|f| {
            let type_ref = match &f.type_ref {
                TypeRef::Named(n) => {
                    if let Some(&p) = aliases.get(n.as_str()) {
                        TypeRef::Builtin(p)
                    } else {
                        f.type_ref.clone()
                    }
                }
                _ => f.type_ref.clone(),
            };
            Field { type_ref, ..f }
        })
        .collect()
}

/// Parse a single XSD document and return all top-level type definitions.
fn parse_document(doc: &roxmltree::Document) -> Result<Vec<TypeDef>> {
    let root = doc.root_element();
    let mut types = Vec::new();

    for child in root.children().filter(|n| n.is_element()) {
        let local = child.tag_name().name();
        match local {
            "simpleType" => {
                if let Some(t) = parse_simple_type(child)? {
                    types.push(TypeDef::Simple(t));
                }
            }
            "complexType" => {
                if let Some(t) = parse_complex_type(child)? {
                    types.push(TypeDef::Complex(t));
                }
            }
            // xs:element at schema root — wrap as a complex type if it has
            // an inline complex type definition.
            "element" => {
                let name = child.attribute("name").unwrap_or("").to_owned();
                if let Some(inner) = find_child_ns(child, XS, "complexType") {
                    if let Some(mut t) = parse_complex_type(inner)? {
                        if t.name.is_empty() {
                            t.name = name;
                        }
                        types.push(TypeDef::Complex(t));
                    }
                }
            }
            // xs:import — we handle cross-file loading at the directory level,
            // not by following schemaLocation here.
            "import" | "include" | "annotation" => {}
            _ => {}
        }
    }

    Ok(types)
}

fn parse_simple_type(node: Node) -> Result<Option<SimpleType>> {
    let name = match node.attribute("name") {
        Some(n) => n.trim().to_owned(),
        None => return Ok(None),
    };

    let restriction = match find_child_ns(node, XS, "restriction") {
        Some(r) => r,
        None => return Ok(None),
    };

    let mut variants = Vec::new();
    let mut next_number: i32 = 0;
    for child in restriction.children().filter(|n| n.is_element()) {
        if child.tag_name().name() == "enumeration" {
            if let Some(raw) = child.attribute("value") {
                let mut variant = parse_enum_variant(raw)?;
                // If the XSD value didn't carry an embedded number, assign
                // one sequentially so every variant gets a unique tag.
                if !raw.contains('=') {
                    variant.number = next_number;
                }
                next_number = variant.number + 1;
                variants.push(variant);
            }
        }
    }

    if variants.is_empty() {
        // A simpleType with no enumerations (e.g. a restricted string/int).
        // Treat as a named string alias for now.
        return Ok(None);
    }

    Ok(Some(SimpleType { name, variants }))
}

/// Parse the "Name=N" convention used in the XSD enum values.
/// Falls back to incrementing index if the convention isn't present.
fn parse_enum_variant(raw: &str) -> Result<EnumVariant> {
    if let Some(eq) = raw.rfind('=') {
        let name = raw[..eq].trim().to_owned();
        let number: i32 = raw[eq + 1..]
            .trim()
            .parse()
            .with_context(|| format!("parsing enum value number in '{raw}'"))?;
        Ok(EnumVariant { name, number })
    } else {
        // No embedded number — use the raw value as the name, number TBD at
        // resolution time. Caller can renumber if needed.
        Ok(EnumVariant {
            name: raw.trim().to_owned(),
            number: 0,
        })
    }
}

fn parse_complex_type(node: Node) -> Result<Option<ComplexType>> {
    let name = node.attribute("name").unwrap_or("").trim().to_owned();

    // xs:sequence
    if let Some(seq) = find_child_ns(node, XS, "sequence") {
        let fields = parse_fields(seq)?;
        return Ok(Some(ComplexType {
            name,
            content: ComplexContent::Sequence(fields),
        }));
    }

    // xs:choice
    if let Some(choice) = find_child_ns(node, XS, "choice") {
        let fields = parse_fields(choice)?;
        return Ok(Some(ComplexType {
            name,
            content: ComplexContent::Choice(fields),
        }));
    }

    // xs:complexContent → xs:extension
    if let Some(cc) = find_child_ns(node, XS, "complexContent") {
        if let Some(ext) = find_child_ns(cc, XS, "extension") {
            let base = ext
                .attribute("base")
                .with_context(|| format!("xs:extension missing base in type '{name}'"))?
                .to_owned();
            // Strip namespace prefix from base name if present.
            let base = strip_ns_prefix(&base).to_owned();

            let extra_fields = if let Some(seq) = find_child_ns(ext, XS, "sequence") {
                parse_fields(seq)?
            } else {
                vec![]
            };

            return Ok(Some(ComplexType {
                name,
                content: ComplexContent::Extension { base, extra_fields },
            }));
        }
    }

    // Empty complex type — emit as an empty struct.
    Ok(Some(ComplexType {
        name,
        content: ComplexContent::Sequence(vec![]),
    }))
}

fn parse_fields(parent: Node) -> Result<Vec<Field>> {
    let mut fields = Vec::new();
    for child in parent.children().filter(|n| n.is_element()) {
        if child.tag_name().name() != "element" {
            continue;
        }
        let name = match child.attribute("name") {
            Some(n) => n.trim().to_owned(),
            None => continue,
        };
        if name.is_empty() {
            continue;
        }

        let type_ref = if let Some(t) = child.attribute("type") {
            resolve_type_ref(t.trim())
        } else if find_child_ns(child, XS, "complexType").is_some()
            || find_child_ns(child, XS, "simpleType").is_some()
        {
            // Inline type definition — treat as a named reference to `name`.
            // The inline type will be lifted to the top level in a follow-up pass
            // if needed; for now reference it by name.
            TypeRef::Named(name.clone())
        } else {
            TypeRef::Builtin(Primitive::String)
        };

        let min_occurs: u32 = child
            .attribute("minOccurs")
            .unwrap_or("1")
            .parse()
            .unwrap_or(1);
        let max_occurs = child.attribute("maxOccurs").unwrap_or("1");

        fields.push(Field {
            name,
            type_ref,
            optional: min_occurs == 0,
            repeated: max_occurs == "unbounded",
        });
    }
    Ok(fields)
}

fn resolve_type_ref(raw: &str) -> TypeRef {
    let name = strip_ns_prefix(raw).trim();
    match name {
        "string" | "normalizedString" | "token" | "NMTOKEN" | "ID" | "IDREF" | "anyURI"
        | "date" | "dateTime" | "time" | "duration" | "gYear" | "gMonth" | "gDay" => {
            TypeRef::Builtin(Primitive::String)
        }
        "boolean" => TypeRef::Builtin(Primitive::Bool),
        "int" | "integer" | "short" | "byte" => TypeRef::Builtin(Primitive::Int32),
        "long" => TypeRef::Builtin(Primitive::Int64),
        "unsignedInt" | "unsignedShort" | "unsignedByte" | "nonNegativeInteger" => {
            TypeRef::Builtin(Primitive::UInt32)
        }
        "unsignedLong" | "positiveInteger" => TypeRef::Builtin(Primitive::UInt64),
        "float" => TypeRef::Builtin(Primitive::Float),
        "double" | "decimal" => TypeRef::Builtin(Primitive::Double),
        "base64Binary" | "hexBinary" => TypeRef::Builtin(Primitive::Bytes),
        // Java/.NET-style array notation sometimes found in vendor XSDs
        "byte[]" | "Byte[]" => TypeRef::Builtin(Primitive::Bytes),
        "string[]" | "String[]" => TypeRef::Builtin(Primitive::String),
        other => TypeRef::Named(other.to_owned()),
    }
}

/// Flatten xs:extension by prepending base type fields into the child.
fn resolve_extensions(types: Vec<TypeDef>, index: &HashMap<String, usize>) -> Result<Vec<TypeDef>> {
    let mut out = Vec::with_capacity(types.len());

    for def in &types {
        match def {
            TypeDef::Complex(ComplexType {
                name,
                content: ComplexContent::Extension { base, extra_fields },
            }) => {
                let base_fields = match index.get(base) {
                    Some(&idx) => match &types[idx] {
                        TypeDef::Complex(b) => fields_of(b),
                        _ => bail!(
                            "type '{}' extends '{}' but '{}' is not a complex type",
                            name,
                            base,
                            base
                        ),
                    },
                    None => {
                        eprintln!(
                            "warning: base type '{}' for '{}' not found; skipping base fields",
                            base, name
                        );
                        vec![]
                    }
                };

                let mut all_fields = base_fields;
                all_fields.extend_from_slice(extra_fields);

                out.push(TypeDef::Complex(ComplexType {
                    name: name.clone(),
                    content: ComplexContent::Sequence(all_fields),
                }));
            }
            other => out.push(other.clone()),
        }
    }

    Ok(out)
}

fn fields_of(ct: &ComplexType) -> Vec<Field> {
    match &ct.content {
        ComplexContent::Sequence(f) | ComplexContent::Choice(f) => f.clone(),
        ComplexContent::Extension { extra_fields, .. } => extra_fields.clone(),
    }
}

fn find_child_ns<'a>(node: Node<'a, '_>, ns: &str, local: &str) -> Option<Node<'a, 'a>> {
    node.children().find(|n| {
        n.is_element() && n.tag_name().name() == local && n.tag_name().namespace() == Some(ns)
    })
}

fn strip_ns_prefix(name: &str) -> &str {
    name.rfind(':').map(|i| &name[i + 1..]).unwrap_or(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Resolve the workspace-relative path to `schemas/synthetic`.
    fn synthetic_dir() -> std::path::PathBuf {
        // CARGO_MANIFEST_DIR = …/crates/spear-gen
        let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        manifest.join("../../schemas/synthetic")
    }

    // ── recursive scanning ────────────────────────────────────────────────────

    /// `collect_xsd_files` must find files in subdirectories.
    /// `schemas/synthetic/sub/credentials.xsd` exists one level below the
    /// root; it must appear in the collected list.
    #[test]
    fn collect_xsd_files_is_recursive() {
        let dir = synthetic_dir();
        let files = collect_xsd_files(&dir).expect("collect failed");

        let names: Vec<_> = files
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap())
            .collect();

        assert!(
            names.contains(&"credentials.xsd"),
            "expected credentials.xsd in {names:?}"
        );
        // Top-level files must still be present.
        assert!(names.contains(&"track.xsd"));
        assert!(names.contains(&"alert.xsd"));
    }

    /// `load_schemas` must include the `Credentials` type defined in the
    /// subdirectory file.
    #[test]
    fn load_schemas_includes_subdirectory_types() {
        let dir = synthetic_dir();
        let types = load_schemas(&dir).expect("load failed");

        let names: Vec<_> = types.iter().map(|t| t.name()).collect();
        assert!(
            names.contains(&"Credentials"),
            "expected Credentials in {names:?}"
        );
    }

    // ── primitive type alias resolution ──────────────────────────────────────

    /// `SecurityToken` is a `xs:restriction base="xs:base64Binary"` alias.
    /// After `load_schemas` the `Token` field on `Credentials` must resolve
    /// to `TypeRef::Builtin(Primitive::Bytes)`, not a bare `Named` ref.
    #[test]
    fn primitive_alias_resolves_to_builtin() {
        let dir = synthetic_dir();
        let types = load_schemas(&dir).expect("load failed");

        let creds = types
            .iter()
            .find(|t| t.name() == "Credentials")
            .expect("Credentials not found");

        let fields = match creds {
            TypeDef::Complex(ct) => match &ct.content {
                ComplexContent::Sequence(f) => f,
                other => panic!("unexpected content: {other:?}"),
            },
            _ => panic!("expected complex type"),
        };

        let token_field = fields
            .iter()
            .find(|f| f.name == "Token")
            .expect("Token field not found");

        assert_eq!(
            token_field.type_ref,
            TypeRef::Builtin(Primitive::Bytes),
            "SecurityToken alias should resolve to Bytes"
        );
    }

    /// `CallSign` is a `xs:restriction base="xs:string"` alias.
    /// The `Label` field must resolve to `TypeRef::Builtin(Primitive::String)`.
    #[test]
    fn string_alias_resolves_to_builtin() {
        let dir = synthetic_dir();
        let types = load_schemas(&dir).expect("load failed");

        let creds = types
            .iter()
            .find(|t| t.name() == "Credentials")
            .expect("Credentials not found");

        let fields = match creds {
            TypeDef::Complex(ct) => match &ct.content {
                ComplexContent::Sequence(f) => f,
                other => panic!("unexpected content: {other:?}"),
            },
            _ => panic!("expected complex type"),
        };

        let label_field = fields
            .iter()
            .find(|f| f.name == "Label")
            .expect("Label field not found");

        assert_eq!(
            label_field.type_ref,
            TypeRef::Builtin(Primitive::String),
            "CallSign alias should resolve to String"
        );
    }

    /// Aliases must NOT produce their own `TypeDef::Simple` entries —
    /// they are transparent to the emitter.
    #[test]
    fn primitive_alias_not_emitted_as_type() {
        let dir = synthetic_dir();
        let types = load_schemas(&dir).expect("load failed");

        let names: Vec<_> = types.iter().map(|t| t.name()).collect();
        assert!(
            !names.contains(&"SecurityToken"),
            "SecurityToken should not appear as a TypeDef (it's an alias)"
        );
        assert!(
            !names.contains(&"CallSign"),
            "CallSign should not appear as a TypeDef (it's an alias)"
        );
    }
}
