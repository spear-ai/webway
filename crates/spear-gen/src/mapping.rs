use crate::parser::ast::Primitive;

pub struct TypeMapping {
    pub proto_type: &'static str,
    pub rust_type: &'static str,
    /// The prost field attribute, e.g. `string` or `int32`.
    pub prost_tag: &'static str,
    /// Fixed byte size in the raw binary encoding.
    /// 0 means variable-length (string, bytes).
    pub byte_size: usize,
}

pub fn map_primitive(p: Primitive) -> TypeMapping {
    match p {
        Primitive::String => TypeMapping {
            proto_type: "string",
            rust_type: "String",
            prost_tag: "string",
            byte_size: 0, // null-terminated, variable length
        },
        Primitive::Bool => TypeMapping {
            proto_type: "bool",
            rust_type: "bool",
            prost_tag: "bool",
            byte_size: 1,
        },
        Primitive::Int32 => TypeMapping {
            proto_type: "int32",
            rust_type: "i32",
            prost_tag: "int32",
            byte_size: 4,
        },
        Primitive::Int64 => TypeMapping {
            proto_type: "int64",
            rust_type: "i64",
            prost_tag: "int64",
            byte_size: 8,
        },
        Primitive::UInt32 => TypeMapping {
            proto_type: "uint32",
            rust_type: "u32",
            prost_tag: "uint32",
            byte_size: 4,
        },
        Primitive::UInt64 => TypeMapping {
            proto_type: "uint64",
            rust_type: "u64",
            prost_tag: "uint64",
            byte_size: 8,
        },
        Primitive::Float => TypeMapping {
            proto_type: "float",
            rust_type: "f32",
            prost_tag: "float",
            byte_size: 4,
        },
        Primitive::Double => TypeMapping {
            proto_type: "double",
            rust_type: "f64",
            prost_tag: "double",
            byte_size: 8,
        },
        Primitive::Bytes => TypeMapping {
            proto_type: "bytes",
            rust_type: "Vec<u8>",
            prost_tag: "bytes",
            byte_size: 0, // variable length
        },
    }
}
