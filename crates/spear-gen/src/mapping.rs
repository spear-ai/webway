use crate::parser::ast::Primitive;

pub struct TypeMapping {
    pub proto_type: &'static str,
    pub rust_type: &'static str,
    /// The prost field attribute, e.g. `string` or `int32`.
    pub prost_tag: &'static str,
}

pub fn map_primitive(p: Primitive) -> TypeMapping {
    match p {
        Primitive::String => TypeMapping {
            proto_type: "string",
            rust_type: "String",
            prost_tag: "string",
        },
        Primitive::Bool => TypeMapping {
            proto_type: "bool",
            rust_type: "bool",
            prost_tag: "bool",
        },
        Primitive::Int32 => TypeMapping {
            proto_type: "int32",
            rust_type: "i32",
            prost_tag: "int32",
        },
        Primitive::Int64 => TypeMapping {
            proto_type: "int64",
            rust_type: "i64",
            prost_tag: "int64",
        },
        Primitive::UInt32 => TypeMapping {
            proto_type: "uint32",
            rust_type: "u32",
            prost_tag: "uint32",
        },
        Primitive::UInt64 => TypeMapping {
            proto_type: "uint64",
            rust_type: "u64",
            prost_tag: "uint64",
        },
        Primitive::Float => TypeMapping {
            proto_type: "float",
            rust_type: "f32",
            prost_tag: "float",
        },
        Primitive::Double => TypeMapping {
            proto_type: "double",
            rust_type: "f64",
            prost_tag: "double",
        },
        Primitive::Bytes => TypeMapping {
            proto_type: "bytes",
            rust_type: "Vec<u8>",
            prost_tag: "bytes",
        },
    }
}
