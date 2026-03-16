use crate::config::{TargetConfig, WordSize};
use crate::ir::CPrimitive;

/// The fully-resolved mapping for a C primitive type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CTypeMapping {
    pub rust_type: &'static str,
    pub proto_type: &'static str,
    pub byte_size: u64,
}

/// Map a `CPrimitive` to concrete types, taking `WordSize` into account for
/// `Long`/`ULong`.
pub fn map_primitive(p: CPrimitive, config: TargetConfig) -> CTypeMapping {
    match p {
        CPrimitive::Char => CTypeMapping {
            rust_type: "i8",
            proto_type: "int32",
            byte_size: 1,
        },
        CPrimitive::UChar => CTypeMapping {
            rust_type: "u8",
            proto_type: "uint32",
            byte_size: 1,
        },
        CPrimitive::Short => CTypeMapping {
            rust_type: "i16",
            proto_type: "int32",
            byte_size: 2,
        },
        CPrimitive::UShort => CTypeMapping {
            rust_type: "u16",
            proto_type: "uint32",
            byte_size: 2,
        },
        CPrimitive::Int => CTypeMapping {
            rust_type: "i32",
            proto_type: "int32",
            byte_size: 4,
        },
        CPrimitive::UInt => CTypeMapping {
            rust_type: "u32",
            proto_type: "uint32",
            byte_size: 4,
        },
        CPrimitive::Long => match config.word_size {
            WordSize::W32 => CTypeMapping {
                rust_type: "i32",
                proto_type: "int32",
                byte_size: 4,
            },
            WordSize::W64 => CTypeMapping {
                rust_type: "i64",
                proto_type: "int64",
                byte_size: 8,
            },
        },
        CPrimitive::ULong => match config.word_size {
            WordSize::W32 => CTypeMapping {
                rust_type: "u32",
                proto_type: "uint32",
                byte_size: 4,
            },
            WordSize::W64 => CTypeMapping {
                rust_type: "u64",
                proto_type: "uint64",
                byte_size: 8,
            },
        },
        CPrimitive::LongLong => CTypeMapping {
            rust_type: "i64",
            proto_type: "int64",
            byte_size: 8,
        },
        CPrimitive::ULongLong => CTypeMapping {
            rust_type: "u64",
            proto_type: "uint64",
            byte_size: 8,
        },
        CPrimitive::Float => CTypeMapping {
            rust_type: "f32",
            proto_type: "float",
            byte_size: 4,
        },
        CPrimitive::Double => CTypeMapping {
            rust_type: "f64",
            proto_type: "double",
            byte_size: 8,
        },
    }
}

/// Return the `from_le_bytes`/`from_be_bytes` decode expression for a
/// primitive type.
///
/// Returns a format string with `{bytes}` as the byte-slice placeholder and
/// `{suffix}` as the endianness suffix.
pub fn decode_expr(rust_type: &str, byte_size: u64, suffix: &str) -> String {
    match byte_size {
        1 => {
            // Single byte — no endianness needed.
            format!("{rust_type}::from_ne_bytes([bytes[_ofs]])")
        }
        _ => {
            let arr_expr = format!("bytes[_ofs.._ofs + {byte_size}].try_into().unwrap()");
            format!("{rust_type}::{suffix}({arr_expr})")
        }
    }
}
