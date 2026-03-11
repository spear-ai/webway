//! XSD parser — reads `.xsd` files from a directory and produces an
//! [`ast::TypeDef`] list that emitters consume.
//!
//! Parsing is two-pass:
//! 1. Every file is parsed into raw [`ast::TypeDef`] values and inserted
//!    into a name index.
//! 2. [`resolve_extensions`] walks the index and flattens `xs:extension`
//!    base-type fields into child types so emitters never see
//!    [`ComplexContent::Extension`] in practice.
//!
//! Cross-file references (a type in `alert.xsd` referencing an enum from
//! `track.xsd`) are handled naturally because all files are loaded before
//! any resolution step runs.

pub mod ast;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use roxmltree::Node;

use ast::*;

const XS: &str = "http://www.w3.org/2001/XMLSchema";

/// Load all XSD files in `dir`, resolve cross-file type references, and
/// return a flat list of all type definitions.
pub fn load_schemas(dir: &Path) -> Result<Vec<TypeDef>> {
    let xsd_files = collect_xsd_files(dir)?;

    // First pass: parse every file into raw TypeDefs. Types may reference
    // names that are defined in other files — we resolve that in pass two.
    let mut all_types: Vec<TypeDef> = Vec::new();
    let mut type_index: HashMap<String, usize> = HashMap::new();

    for path in &xsd_files {
        let src = std::fs::read_to_string(path)
            .with_context(|| format!("reading {}", path.display()))?;
        let doc = roxmltree::Document::parse(&src)
            .with_context(|| format!("parsing {}", path.display()))?;

        let defs = parse_document(&doc)
            .with_context(|| format!("extracting types from {}", path.display()))?;

        for def in defs {
            let name = def.name().to_owned();
            let idx = all_types.len();
            all_types.push(def);
            type_index.insert(name, idx);
        }
    }

    // Second pass: resolve xs:extension base names.
    // For v1 we flatten extension fields inline — find the base type and
    // prepend its fields to the child's extra_fields.
    let resolved = resolve_extensions(all_types, &type_index)?;

    Ok(resolved)
}

fn collect_xsd_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for entry in std::fs::read_dir(dir)
        .with_context(|| format!("reading directory {}", dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("xsd") {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
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
        Some(n) => n.to_owned(),
        None => return Ok(None),
    };

    let restriction = match find_child_ns(node, XS, "restriction") {
        Some(r) => r,
        None => return Ok(None),
    };

    let mut variants = Vec::new();
    for child in restriction.children().filter(|n| n.is_element()) {
        if child.tag_name().name() == "enumeration" {
            if let Some(raw) = child.attribute("value") {
                variants.push(parse_enum_variant(raw)?);
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
    let name = node.attribute("name").unwrap_or("").to_owned();

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
            Some(n) => n.to_owned(),
            None => continue,
        };

        let type_ref = if let Some(t) = child.attribute("type") {
            resolve_type_ref(t)
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
    let name = strip_ns_prefix(raw);
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
        other => TypeRef::Named(other.to_owned()),
    }
}

/// Flatten xs:extension by prepending base type fields into the child.
fn resolve_extensions(
    types: Vec<TypeDef>,
    index: &HashMap<String, usize>,
) -> Result<Vec<TypeDef>> {
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
                            name, base, base
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
