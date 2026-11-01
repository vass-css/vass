//! Compile command
//!
//! Reads a `.vass`/`.vcss` file, parses, evaluates and writes CSS.
//! The core pipeline is exposed as [`compile_file`] so `watch` can reuse it.

use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use clap::Args as ClapArgs;
use vass_transpiler::OutputStyle;

/// Compile a VASS file to CSS
#[derive(ClapArgs, Debug)]
pub struct Args {
  /// Input VASS file
  #[arg(value_name = "INPUT")]
  pub input: String,

  /// Output CSS file (stdout if not specified)
  #[arg(short, long, value_name = "OUTPUT")]
  pub output: Option<String>,

  /// Generate source map
  #[arg(long)]
  pub source_map: bool,

  /// Minify output
  #[arg(long)]
  pub minify: bool,

  /// Output style (expanded, compressed)
  #[arg(long, default_value = "expanded")]
  pub style: String,
}

/// Run the compile command
pub async fn run(args: Args) -> anyhow::Result<()> {
  let input = PathBuf::from(&args.input);

  // Fail LOUDLY instead of silently accepting a no-op: the transpiler's
  // source_map module is a documented placeholder ("arrive in a later
  // phase"), so requesting a map must be a visible error, not a lie.
  if args.source_map {
    anyhow::bail!(
      "--source-map is not supported yet: source map generation arrives \
             in a later phase (crates/vass-transpiler/src/source_map.rs)"
    );
  }

  let style = if args.minify {
    OutputStyle::Compressed
  } else {
    super::parse_style(&args.style)?
  };

  match &args.output {
    Some(path) => {
      let out = PathBuf::from(path);
      let bytes = compile_file(&input, Some(&out), style)?;
      println!("compiled {} -> {} ({} bytes)", args.input, path, bytes);
    }
    None => {
      compile_file(&input, None, style)?;
    }
  }

  Ok(())
}

/// Compile `input` once and write CSS to `output` (or stdout if `None`).
///
/// Returns the number of bytes written. Shared by `compile` and `watch`.
pub fn compile_file(
  input: &Path,
  output: Option<&Path>,
  style: OutputStyle,
) -> anyhow::Result<usize> {
  // Read the source
  let source = vass_io::read_to_string(input).map_err(|e| anyhow::anyhow!("{}", e))?;

  // Pick the syntax from the extension
  let syntax = vass_parser::detect_syntax(input);

  // Parse
  let sheet = vass_parser::parse(&source, syntax).map_err(|e| render_error(input, &e))?;

  // Evaluate to CSS (with module loading support)
  let options = vass_evaluator::EvalOptions {
    entry: Some(input.to_path_buf()),
    load_paths: Vec::new(),
    loader: None,
  };
  let css = vass_evaluator::evaluate_with(&sheet, options).map_err(|e| render_error(input, &e))?;

  // Render
  let rendered = vass_transpiler::render(&css, style);

  match output {
    Some(path) => {
      if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
      {
        std::fs::create_dir_all(parent)?;
      }
      std::fs::write(path, &rendered)?;
    }
    None => {
      print!("{}", rendered);
    }
  }

  Ok(rendered.len())
}

/// Format an error as an ESLint-crisp block: a heavy rule, a
/// `file 🔥 1 error 🔥` headline, the offending source line with an `↑`
/// caret under the exact column and a `└─ ✖ message [ code ]` row, plus
/// any help/notes. Falls back to the message-only layout when the source
/// can't be re-read (virtual/generated inputs).
///
/// Shared by `compile`, `watch` and `build`.
pub(crate) fn render_error(file: &Path, err: &vass_core::error::Error) -> anyhow::Error {
  let mut e = err.clone();
  if e.file.is_none() {
    e.file = Some(file.display().to_string());
  }
  let color = vass_core::error::color_enabled(std::io::stderr().is_terminal());
  let text =
    vass_core::error::render::render(&e, &|path| vass_io::read_to_string(path).ok(), color);
  anyhow::anyhow!("{}", text.trim_end())
}
