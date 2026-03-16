use std::collections::HashMap;

/// All structs discovered from the parsed headers.
pub type Registry = HashMap<String, CStruct>;

/// A C struct with its fields and total byte size (as reported by libclang,
/// including alignment padding).
#[derive(Debug, Clone, PartialEq)]
pub struct CStruct {
    pub name: String,
    pub fields: Vec<CField>,
    /// True ABI size in bytes, including alignment padding.
    /// Comes from `clang::Type::get_sizeof()`.
    pub total_byte_size: u64,
}

/// A single field inside a C struct.
#[derive(Debug, Clone, PartialEq)]
pub struct CField {
    pub name: String,
    pub ty: CTypeRef,
    /// `Some(width)` when this is a bitfield.
    pub bitfield_width: Option<u32>,
}

/// The type of a field, as resolved by the parser.
#[derive(Debug, Clone, PartialEq)]
pub enum CTypeRef {
    Primitive(CPrimitive),
    /// Fixed-size array: element type × count.
    Array(Box<CTypeRef>, u64),
    /// Reference to another struct by name (canonical).
    Struct(String),
    /// Union — we don't decode it; store as raw bytes.
    Union {
        byte_size: u64,
    },
    /// A type that could not be resolved (e.g., forward declaration only).
    Unresolved(String),
}

/// C arithmetic types, before word-size resolution.
///
/// `Long`/`ULong` are ambiguous until `WordSize` is known.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CPrimitive {
    Char,      // signed char  → i8
    UChar,     // unsigned char → u8
    Short,     // i16
    UShort,    // u16
    Int,       // i32
    UInt,      // u32
    Long,      // i32 (W32) or i64 (W64)
    ULong,     // u32 (W32) or u64 (W64)
    LongLong,  // i64
    ULongLong, // u64
    Float,     // f32
    Double,    // f64
}
