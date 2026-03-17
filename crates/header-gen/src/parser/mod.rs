mod discovery;
mod traversal;

use std::path::Path;

use anyhow::Result;

use crate::config::TargetConfig;
use crate::ir::Registry;
use crate::report::ReviewReport;

pub use discovery::discover;

/// Full parse pipeline:
/// 1. Walk `input_dir` to discover headers + include flags.
/// 2. Parse them all as a single umbrella translation unit.
///
/// Returns the struct registry and review report.
pub fn parse(
    input_dir: &Path,
    extra_includes: &[String],
    defines: &[String],
    config: TargetConfig,
) -> Result<(Registry, ReviewReport)> {
    let (headers, mut include_flags) = discovery::discover(input_dir)?;

    if headers.is_empty() {
        return Ok((Registry::new(), ReviewReport::default()));
    }

    // Merge caller-supplied include paths with the auto-discovered ones.
    include_flags.extend_from_slice(extra_includes);

    traversal::parse_headers(&headers, input_dir, &include_flags, defines, config)
}
