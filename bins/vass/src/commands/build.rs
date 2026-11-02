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

    match vass_compile(entry, &out, style) {
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

fn vass_compile(input: &Path, output: &Path, style: OutputStyle) -> anyhow::Result<usize> {
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

#[cfg(test)]
mod tests {
  use super::*;
  use crate::commands::parse_style;

  #[test]
  fn test_is_entry_stylesheet_vass() {
    let p = std::path::Path::new("main.vass");
    assert!(is_entry_stylesheet(p));
  }

  #[test]
  fn test_is_entry_stylesheet_vcss() {
    let p = std::path::Path::new("main.vcss");
    assert!(is_entry_stylesheet(p));
  }

  #[test]
  fn test_is_entry_stylesheet_partial() {
    let p = std::path::Path::new("_partial.vass");
    assert!(!is_entry_stylesheet(p));
  }

  #[test]
  fn test_is_entry_stylesheet_other_ext() {
    let p = std::path::Path::new("main.css");
    assert!(!is_entry_stylesheet(p));
  }

  #[test]
  fn test_is_entry_stylesheet_no_name() {
    let p = std::path::Path::new(".");
    assert!(!is_entry_stylesheet(p));
  }

  #[tokio::test]
  async fn test_build_run_invalid_style() {
    let result = run(Args {
      dir: ".nonexistent_dir_for_test".to_string(),
      out_dir: "dist".to_string(),
      style: "expanded".to_string(),
    })
    .await;
    assert!(result.is_err());
  }

  #[tokio::test]
  async fn test_build_no_entry_stylesheets() {
    let dir = std::env::temp_dir().join(format!("vass-build-empty-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("_partial.vass"), "$x: 1\n").unwrap();
    let result = run(Args {
      dir: dir.to_string_lossy().to_string(),
      out_dir: "dist".to_string(),
      style: "expanded".to_string(),
    })
    .await;
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("no entry stylesheets"), "got: {msg}");
    std::fs::remove_dir_all(&dir).ok();
  }

  #[tokio::test]
  async fn test_build_compile_error() {
    let dir = std::env::temp_dir().join(format!("vass-build-err-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("bad.vass"), "@error \"boom\"\n").unwrap();
    let result = run(Args {
      dir: dir.to_string_lossy().to_string(),
      out_dir: "dist".to_string(),
      style: "expanded".to_string(),
    })
    .await;
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("failed to build"), "got: {msg}");
    std::fs::remove_dir_all(&dir).ok();
  }

  #[test]
  fn test_parse_style_expanded() {
    assert!(parse_style("expanded").is_ok());
  }

  #[test]
  fn test_parse_style_compressed() {
    assert!(parse_style("compressed").is_ok());
  }

  #[test]
  fn test_parse_style_invalid() {
    assert!(parse_style("invalid").is_err());
  }
}
