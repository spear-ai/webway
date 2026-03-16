use std::fmt::Write as FmtWrite;

/// Items that require human review — bitfields, unions, unresolved types, and
/// clang parse errors.  Written to `<out-rust>/review_report.txt`.
#[derive(Debug, Default)]
pub struct ReviewReport {
    pub bitfields: Vec<BitfieldItem>,
    pub unions: Vec<UnionItem>,
    pub unresolved: Vec<UnresolvedItem>,
    pub parse_failures: Vec<ParseFailure>,
}

#[derive(Debug)]
pub struct BitfieldItem {
    pub struct_name: String,
    pub field_name: String,
    pub width: u32,
}

#[derive(Debug)]
pub struct UnionItem {
    pub struct_name: String,
    pub field_name: String,
    pub byte_size: u64,
}

#[derive(Debug)]
pub struct UnresolvedItem {
    pub struct_name: String,
    pub field_name: String,
    pub type_name: String,
}

#[derive(Debug)]
pub struct ParseFailure {
    pub file: String,
    pub message: String,
}

impl ReviewReport {
    pub fn is_empty(&self) -> bool {
        self.bitfields.is_empty()
            && self.unions.is_empty()
            && self.unresolved.is_empty()
            && self.parse_failures.is_empty()
    }

    pub fn render(&self) -> String {
        let mut out = String::new();

        if self.is_empty() {
            out.push_str("No issues found.\n");
            return out;
        }

        if !self.bitfields.is_empty() {
            out.push_str("=== BITFIELDS (omitted from proto/mapping) ===\n");
            for b in &self.bitfields {
                writeln!(
                    out,
                    "  struct {} :: {} : {} bits",
                    b.struct_name, b.field_name, b.width
                )
                .unwrap();
            }
            out.push('\n');
        }

        if !self.unions.is_empty() {
            out.push_str("=== UNIONS (stored as raw bytes) ===\n");
            for u in &self.unions {
                writeln!(
                    out,
                    "  struct {} :: {} ({} bytes)",
                    u.struct_name, u.field_name, u.byte_size
                )
                .unwrap();
            }
            out.push('\n');
        }

        if !self.unresolved.is_empty() {
            out.push_str("=== UNRESOLVED TYPES ===\n");
            for u in &self.unresolved {
                writeln!(
                    out,
                    "  struct {} :: {} -> {}",
                    u.struct_name, u.field_name, u.type_name
                )
                .unwrap();
            }
            out.push('\n');
        }

        if !self.parse_failures.is_empty() {
            out.push_str("=== PARSE FAILURES ===\n");
            for p in &self.parse_failures {
                writeln!(out, "  {} : {}", p.file, p.message).unwrap();
            }
        }

        out
    }
}
