/// Runtime target configuration — endianness and pointer/long size.
///
/// These flow through the parser (as clang flags) and all three emitters
/// (to pick `from_le_bytes`/`from_be_bytes` and `i32`/`i64` for `long`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetConfig {
    pub endian: Endian,
    pub word_size: WordSize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endian {
    Little,
    Big,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordSize {
    W32,
    W64,
}

impl TargetConfig {
    /// Build the clang compiler flags that correspond to this target config
    /// plus any user-supplied includes/defines.
    pub fn clang_flags(&self, include_dirs: &[String], defines: &[String]) -> Vec<String> {
        let mut flags = Vec::new();

        match self.word_size {
            WordSize::W32 => flags.push("-m32".to_owned()),
            WordSize::W64 => {} // native default
        }

        if self.endian == Endian::Big {
            flags.push("-mbig-endian".to_owned());
        }

        for d in defines {
            flags.push(format!("-D{d}"));
        }

        for i in include_dirs {
            flags.push(format!("-I{i}"));
        }

        flags
    }

    /// The `from_*_bytes` suffix used in decode expressions.
    pub fn from_bytes_suffix(&self) -> &'static str {
        match self.endian {
            Endian::Little => "from_le_bytes",
            Endian::Big => "from_be_bytes",
        }
    }
}

impl std::fmt::Display for Endian {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Endian::Little => write!(f, "little"),
            Endian::Big => write!(f, "big"),
        }
    }
}

impl std::fmt::Display for WordSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WordSize::W32 => write!(f, "32"),
            WordSize::W64 => write!(f, "64"),
        }
    }
}
