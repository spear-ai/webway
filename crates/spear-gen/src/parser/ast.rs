/// Internal representation of XSD types after parsing.
/// All emitters (proto, rust) work from this AST.
#[derive(Debug, Clone)]
pub enum TypeDef {
    Simple(SimpleType),
    Complex(ComplexType),
}

impl TypeDef {
    pub fn name(&self) -> &str {
        match self {
            TypeDef::Simple(t) => &t.name,
            TypeDef::Complex(t) => &t.name,
        }
    }
}

/// xs:simpleType — in practice this is always an enumeration in our XSDs.
#[derive(Debug, Clone)]
pub struct SimpleType {
    pub name: String,
    pub variants: Vec<EnumVariant>,
}

#[derive(Debug, Clone)]
pub struct EnumVariant {
    pub name: String,
    /// Parsed from the "Name=N" convention in the XSD value attribute.
    pub number: i32,
}

/// xs:complexType
#[derive(Debug, Clone)]
pub struct ComplexType {
    pub name: String,
    pub content: ComplexContent,
}

#[derive(Debug, Clone)]
pub enum ComplexContent {
    /// xs:sequence — ordered list of fields.
    Sequence(Vec<Field>),
    /// xs:choice — at most one field is present.
    /// v1: emitted as all-optional fields with a comment noting the constraint.
    Choice(Vec<Field>),
    /// xs:extension — base type fields are flattened into this type.
    Extension {
        base: String,
        extra_fields: Vec<Field>,
    },
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub type_ref: TypeRef,
    /// false = required (minOccurs >= 1), true = optional (minOccurs = 0)
    pub optional: bool,
    /// true when maxOccurs="unbounded" — maps to Vec<T> / repeated
    pub repeated: bool,
}

#[derive(Debug, Clone)]
pub enum TypeRef {
    /// Built-in XSD primitive (xs:string, xs:int, etc.)
    Builtin(Primitive),
    /// Reference to a named type defined elsewhere in the schema.
    Named(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Primitive {
    String,
    Bool,
    Int32,
    Int64,
    UInt32,
    UInt64,
    Float,
    Double,
    Bytes,
}
