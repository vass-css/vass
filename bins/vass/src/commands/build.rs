//! `vass build` — compile every entry stylesheet of a directory tree.
//!
//! Entry stylesheets are files whose name does not start with `_`
//! (partials are pulled in via `@use`/`@forward`/`@import` only). Each
//! `foo.vass` becomes `foo.css` under the output directory, preserving
//! relative paths.

use std::path::{Path, PathBuf};

use clap::Args as ClapArgs;
use vass_transpiler::OutputStyle;

/// Build all stylesheets in a directory
#[derive(ClapArgs, Debug)]
pub struct Args {
  /// Source directory containing .vass/.vcss files
  #[arg(value_name = "DIR", default_value = ".")]
  pub dir: String,

  /// Output directory for compiled CSS
  #[arg(long, short, value_name = "OUT", default_value = "dist")]
  pub out_dir: String,

  /// Output style (expanded, compressed)
  #[arg(long, default_value = "expanded")]
  pub style: String,
}

/// Run the build command
pub async fn run(args: Args) -> anyhow::Result<()> {
  let src = PathBuf::from(&args.dir);
  if !src.is_dir() {
    anyhow::bail!("`{}` is not a directory", src.display());
  }

  let mut entries = collect_entries(&src)?;
  entries.sort();
  if entries.is_empty() {
    anyhow::bail!("no entry stylesheets found under `{}`", src.display());
  }

  let style = super::parse_style(&args.style)?;

  let mut built = 0usize;
  let mut errors = 0usize;
  for entry in &entries {
    let rel = entry.strip_prefix(&src).unwrap_or(entry.as_path());
    let out = PathBuf::from(&args.out_dir).join(rel).with_extension("css");

    match bass_compile(entry, &out, style) {
      Ok(bytes) => {
        println!(
          "built {} -> {} ({bytes} bytes)",
          entry.display(),
          out.display()
        );
        built += 1;
      }
      Err(e) => {
        // `e` already carries the full crisp block with the
        // `file 🔥 N errors 🔥` headline naming the failed stylesheet
        eprintln!("{e}");
        errors += 1;
      }
    }
  }

  if errors > 0 {
    anyhow::bail!("{errors} of {} file(s) failed to build", entries.len());
  }
  println!("built {built} stylesheet(s) into `{}`", args.out_dir);
  Ok(())
}

/// Recursively collect entry stylesheets (non-partials) under `dir`.
fn collect_entries(dir: &Path) -> anyhow::Result<Vec<PathBuf>> {
  let mut out = Vec::new();
  for e in std::fs::read_dir(dir)? {
    let p = e?.path();
    if p.is_dir() {
      out.extend(collect_entries(&p)?);
    } else if is_entry_stylesheet(&p) {
      out.push(p);
    }
  }
  Ok(out)
}

fn is_entry_stylesheet(p: &Path) -> bool {
  let Some(name) = p.file_name().and_then(|n| n.to_str()) else {
    return false;
  };
  if name.starts_with('_') {
    return false;
  }
  matches!(
    p.extension().and_then(|e| e.to_str()),
    Some("vass") | Some("vcss")
  )
}

fn bass_compile(input: &Path, output: &Path, style: OutputStyle) -> anyhow::Result<usize> {
  let source = vass_io::read_to_string(input).map_err(|e| anyhow::anyhow!("{}", e))?;
  let syntax = vass_parser::detect_syntax(input);
  // Same rustc-style diagnostic blocks as `vass compile` (snippet + caret
  // + help) so every command reports errors identically.
  let sheet =
    vass_parser::parse(&source, syntax).map_err(|e| super::compile::render_error(input, &e))?;
  let options = vass_evaluator::EvalOptions {
    entry: Some(input.to_path_buf()),
    load_paths: Vec::new(),
    loader: None,
  };
  let css = vass_evaluator::evaluate_with(&sheet, options)
    .map_err(|e| super::compile::render_error(input, &e))?;
  let rendered = vass_transpiler::render(&css, style);

  if let Some(parent) = output
    .parent()
    .filter(|parent| !parent.as_os_str().is_empty())
  {
    std::fs::create_dir_all(parent)?;
  }
  std::fs::write(output, &rendered)?;
  Ok(rendered.len())
}
