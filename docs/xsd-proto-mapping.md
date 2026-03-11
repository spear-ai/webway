# XSD → Proto / Rust Mapping Rules (v1)

This document is the authoritative reference for how `spear-gen` translates
XSD constructs into proto3 and Rust output. It covers the decisions made,
the rationale, and the known v1 limitations that will need addressing before
any consumer-facing schema versioning bump.

---

## Primitive type table

| XSD type | Proto3 type | Rust type | Notes |
|---|---|---|---|
| `xs:string` | `string` | `String` | |
| `xs:normalizedString` | `string` | `String` | |
| `xs:token` | `string` | `String` | |
| `xs:boolean` | `bool` | `bool` | |
| `xs:int` | `int32` | `i32` | |
| `xs:integer` | `int32` | `i32` | |
| `xs:short` | `int32` | `i32` | |
| `xs:byte` | `int32` | `i32` | |
| `xs:long` | `int64` | `i64` | |
| `xs:unsignedInt` | `uint32` | `u32` | |
| `xs:unsignedShort` | `uint32` | `u32` | |
| `xs:unsignedByte` | `uint32` | `u32` | |
| `xs:unsignedLong` | `uint64` | `u64` | |
| `xs:float` | `float` | `f32` | |
| `xs:double` | `double` | `f64` | |
| `xs:decimal` | `double` | `f64` | Precision loss possible for very large decimals |
| `xs:base64Binary` | `bytes` | `Vec<u8>` | |
| `xs:hexBinary` | `bytes` | `Vec<u8>` | |
| `xs:dateTime` | `string` | `String` | Timestamps preserved as strings in v1; no parsing |
| `xs:date` | `string` | `String` | |
| `xs:time` | `string` | `String` | |
| `xs:anyURI` | `string` | `String` | |
| `xs:ID` / `xs:IDREF` | `string` | `String` | |

---

## Structural constructs

### `xs:simpleType` with `xs:enumeration`

The XSD source uses a non-standard convention where the integer value is
embedded in the enumeration string:

```xml
<xs:enumeration value="Axis_unknown=0"/>
<xs:enumeration value="Axis_X=1"/>
```

`spear-gen` splits on `=` to extract the variant name (`Axis_unknown`) and
its integer value (`0`).

**Proto output:** Standard proto3 enum. Field names are converted to
`SCREAMING_SNAKE_CASE`.

```proto
enum Axis {
  AXIS_UNKNOWN = 0;
  AXIS_X = 1;
}
```

**Rust output:** `#[repr(i32)]` enum with `prost::Enumeration` and
`serde::Deserialize`. The serde rename preserves the original XSD value
string for XML decode compatibility.

```rust
#[derive(prost::Enumeration)]
#[repr(i32)]
pub enum Axis {
    #[serde(rename = "Axis_unknown")]
    AxisUnknown = 0,
    #[serde(rename = "Axis_X")]
    AxisX = 1,
}
```

> **Note:** If an enumeration value does not contain `=`, `spear-gen` emits
> the raw string as the variant name with value `0` and logs a warning.
> Update the XSD to add explicit integer values before relying on this in
> production.

---

### `xs:complexType` with `xs:sequence`

The common case. All child `xs:element` entries become fields in order.
Proto field tags are assigned sequentially starting at `1`.

**Cardinality:**

| XSD attribute | Proto | Rust |
|---|---|---|
| `minOccurs="1"` (default) | singular | `T` (required) |
| `minOccurs="0"` | singular | `Option<T>` |
| `maxOccurs="unbounded"` | `repeated` | `Vec<T>` |

> **Important:** Changing field order or inserting fields in the middle of a
> sequence will shift proto field tags and break wire compatibility. Always
> append new fields to the end of a sequence and bump `schema_version`.

---

### `xs:complexType` with `xs:choice`

`xs:choice` means at most one of the listed elements is present.

**Proto output (v1):** Emitted as a `oneof` block inside the message.

```proto
message AlertSource {
  oneof alert_source_oneof {
    string system_id = 1;
    string sensor_id = 2;
    string operator_id = 3;
  }
}
```

**Rust output (v1):** Emitted as individual fields, all non-optional, with a
comment noting the constraint. This is a v1 simplification — the generated
Rust type will accept a struct with all fields populated even though the XSD
only allows one. The proto `oneof` correctly enforces the constraint on the
wire.

> **Known limitation:** The Rust struct for `xs:choice` types does not
> enforce the "at most one" constraint at the type level. A future v2 pass
> should emit a Rust `enum` for choice types to provide compile-time safety.

---

### `xs:extension` (inheritance)

`xs:extension` is flattened: base type fields are prepended to the child
type's fields. No Rust trait inheritance is used.

```xml
<!-- Base -->
<xs:complexType name="BaseMessage">
  <xs:sequence>
    <xs:element name="MessageId" type="xs:string"/>
    <xs:element name="Timestamp" type="xs:dateTime"/>
  </xs:sequence>
</xs:complexType>

<!-- Child -->
<xs:complexType name="StatusMessage">
  <xs:complexContent>
    <xs:extension base="BaseMessage">
      <xs:sequence>
        <xs:element name="State" type="SystemState"/>
      </xs:sequence>
    </xs:extension>
  </xs:complexContent>
</xs:complexType>
```

**Output** (both proto and Rust):

```proto
message StatusMessage {
  string message_id = 1;   // from BaseMessage
  string timestamp = 2;    // from BaseMessage
  SystemState state = 3;   // from extension
}
```

`BaseMessage` is still emitted as its own type. The extension flattening only
affects the child — it does not remove the base from the output.

> **Known limitation:** If a base type is defined in a file not included in
> the input directory, `spear-gen` logs a warning and skips the base fields.
> Ensure all referenced XSD files are in the input directory.

---

### Cross-file references

All `.xsd` files in the input directory are loaded in a single pass before
any type resolution runs. A type defined in `track.xsd` can be referenced by
name in `alert.xsd` without any explicit `xs:import` handling — as long as
both files are in the same directory.

`xs:import` and `xs:include` elements are recognized and ignored (the
directory scan handles loading). `schemaLocation` attributes are not
followed.

---

### Namespace prefixes

Namespace prefixes on type references (e.g. `tns:TrackCategory`) are stripped
before lookup. The local name is used for all type resolution. This means
types from different namespaces will collide if they share a local name —
not expected to be an issue for these XSD files but worth noting.

---

## Naming conventions

| XSD name | Proto field name | Rust field name |
|---|---|---|
| `TrackId` | `track_id` | `track_id` |
| `LatitudeDeg` | `latitude_deg` | `latitude_deg` |
| `Type` | `type` | `type_field` ← keyword escape |

XSD element names are preserved exactly via `#[serde(rename)]` on Rust
fields so that XML deserialization works against the original element names.

---

## v1 known limitations

These are documented design decisions accepted for v1 of the demo. They
should be addressed before the schema is used in production with real
consumer teams.

| Limitation | Impact | Resolution path |
|---|---|---|
| `xs:choice` emits non-optional struct fields in Rust | No compile-time enforcement of "at most one" | Emit Rust `enum` for choice types in v2 |
| Timestamps emitted as `string` | No normalized time representation | Decide on `int64` (epoch ms) or `google.protobuf.Timestamp` in v2 |
| `xs:decimal` maps to `f64` | Precision loss for values > 2^53 | Use `string` encoding or a decimal library if needed |
| All types emitted into one file | Large schemas produce one large file | Split per source XSD file in a future pass |
| Inline anonymous types not lifted | An inline `xs:complexType` inside an element falls back to a self-referencing named type | Lift anonymous types to top level with generated names |
