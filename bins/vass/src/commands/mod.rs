//! CLI commands

pub mod build;
pub mod check;
pub mod compile;
pub mod completions;
pub mod doctor;
pub mod exec;
pub mod explain;
pub mod fmt;
pub mod init;
pub mod lint;
pub mod list;
pub mod lsp;
pub mod plugin;
pub mod repl;
pub mod stats;
pub mod version;
pub mod watch;

use vass_transpiler::OutputStyle;

/// Parse the shared `--style` option value. Unknown styles are rejected
/// with a clear error instead of silently falling back to expanded output.
pub(crate) fn parse_style(value: &str) -> anyhow::Result<OutputStyle> {
  match value {
    "expanded" => Ok(OutputStyle::Expanded),
    "compressed" => Ok(OutputStyle::Compressed),
    other => anyhow::bail!("unknown --style `{other}` (expected `expanded` or `compressed`)"),
  }
}
