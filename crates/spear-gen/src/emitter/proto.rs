//! Proto3 emitter — walks the AST and produces a `.proto` file.
//!
//! ## Mapping summary
//!
//! | XSD construct | Proto output |
//! |---|---|
//! | `xs:simpleType` with enumerations | `enum` |
//! | `xs:complexType` / `xs:sequence` | `message` |
//! | `xs:choice` | `message` with `oneof` |
//! | `xs:extension` | flattened `message` (base fields first) |
//! | `maxOccurs="unbounded"` | `repeated` field |
//! | `minOccurs="0"` | singular field (proto3 fields are implicitly optional) |
//!
//! Field names are converted to `snake_case`. Enum variant names are
//! converted to `SCREAMING_SNAKE_CASE` as required by proto3.

use crate::mapping::map_primitive;
use crate::parser::ast::*;

/// Emit a `.proto` file (proto3) from a list of type definitions.
pub fn emit(types: &[TypeDef]) -> String {
    let mut out = String::new();
    out.push_str("syntax = \"proto3\";\n\n");

    for def in types {
        match def {
            TypeDef::Simple(t) => emit_enum(t, &mut out),
            TypeDef::Complex(t) => emit_message(t, types, &mut out),
        }
        out.push('\n');
    }

    out
}

fn emit_enum(t: &SimpleType, out: &mut String) {
    out.push_str(&format!("enum {} {{\n", t.name));
    for v in &t.variants {
        // Proto enum field names are conventionally SCREAMING_SNAKE_CASE.
        let field_name = screaming_snake(&v.name);
        out.push_str(&format!("  {} = {};\n", field_name, v.number));
    }
    out.push_str("}\n");
}

fn emit_message(t: &ComplexType, all_types: &[TypeDef], out: &mut String) {
    out.push_str(&format!("message {} {{\n", t.name));

    match &t.content {
        ComplexContent::Sequence(fields) => {
            emit_fields(fields, all_types, out, false);
        }
        ComplexContent::Choice(fields) => {
            out.push_str("  // xs:choice — at most one field is set\n");
            out.push_str(&format!("  oneof {}_oneof {{\n", snake_case(&t.name)));
            let mut tag = 1u32;
            for f in fields {
                let type_str = field_proto_type(&f.type_ref, all_types);
                out.push_str(&format!(
                    "    {} {} = {};\n",
                    type_str,
                    snake_case(&f.name),
                    tag
                ));
                tag += 1;
            }
            out.push_str("  }\n");
        }
        // Extensions are flattened by the resolver before we get here.
        ComplexContent::Extension { .. } => {
            out.push_str("  // unresolved extension\n");
        }
    }

    out.push_str("}\n");
}

fn emit_fields(fields: &[Field], all_types: &[TypeDef], out: &mut String, _in_oneof: bool) {
    let mut tag = 1u32;
    for f in fields {
        let type_str = if f.repeated {
            format!("repeated {}", field_proto_type(&f.type_ref, all_types))
        } else {
            // In proto3 all singular fields are implicitly optional.
            field_proto_type(&f.type_ref, all_types)
        };
        out.push_str(&format!(
            "  {} {} = {};\n",
            type_str,
            snake_case(&f.name),
            tag
        ));
        tag += 1;
    }
}

fn field_proto_type(type_ref: &TypeRef, all_types: &[TypeDef]) -> String {
    match type_ref {
        TypeRef::Builtin(p) => map_primitive(*p).proto_type.to_owned(),
        TypeRef::Named(name) => {
            // Verify the name exists in our type set so we catch dangling refs.
            if all_types.iter().any(|t| t.name() == name) {
                name.clone()
            } else {
                // Unknown external type — fall back to string with a comment.
                format!("string /* unresolved: {name} */")
            }
        }
    }
}

// ── name helpers ──────────────────────────────────────────────────────────────

fn snake_case(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push('_');
        }
        out.push(c.to_lowercase().next().unwrap());
    }
    out
}

fn screaming_snake(s: &str) -> String {
    snake_case(s).to_uppercase()
}
