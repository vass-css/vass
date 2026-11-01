//! `vass check` — validate files for errors without emitting CSS.
//!
//! The default output is the ESLint-crisp layout: one block per file that
//! has problems, with `🔥 N errors 🔥` headlines, source frames and `↑`
//! carets. Directories are accepted and walked recursively, so `vass
//! check .` lints a whole project the way `eslint .` does. `--format
//! simple` keeps the old OK/ERROR lines for CI log parsers.

use std::collections::BTreeMap;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use clap::{Args as ClapArgs, ValueEnum};
use vass_core::error::{Diagnostic, Error, Severity, color_enabled};

/// Output layout for `check`
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum Format {
  /// ESLint-crisp blocks (default)
  Crisp,
  /// Legacy `OK`/`ERROR` one-liners
  Simple,
}

/// Check files for errors without writing output
#[derive(ClapArgs, Debug)]
pub struct Args {
  /// Input files or directories (directories are walked recursively)
  #[arg(required = true, value_name = "FILES")]
  pub files: Vec<String>,

  /// Only parse (syntax check), skip evaluation
  #[arg(long)]
  pub parse_only: bool,

  /// Output format (`crisp` or `simple`)
  #[arg(long, value_enum, default_value = "crisp")]
  pub format: Format,
}

/// Run the check command
pub async fn run(args: Args) -> anyhow::Result<()> {
  // ── expand inputs: files stay, directories are walked recursively ──
  let mut paths: Vec<PathBuf> = Vec::new();
  for f in &args.files {
    let p = PathBuf::from(f);
    if p.is_dir() {
      walk_dir(&p, &mut paths)?;
    } else {
      if !p.exists() {
        anyhow::bail!("`{}` does not exist", p.display());
      }
      paths.push(p);
    }
  }
  paths.sort();
  paths.dedup();
  if paths.is_empty() {
    anyhow::bail!(
      "no `.vass`/`.vcss` files found in the given paths \
             (directories are searched recursively)"
    );
  }

  // ── collect diagnostics per file ──────────────────────────────────
  // A single eval error can point INTO an `@use`d module, so every
  // diagnostic is grouped by the file it actually names — that way the
  // report shows the offending module's block, exactly where the fix
  // belongs.
  let mut groups: BTreeMap<String, Vec<Diagnostic>> = BTreeMap::new();
  for p in &paths {
    for d in check_file(p, args.parse_only) {
      let key = d.file.clone().unwrap_or_else(|| p.display().to_string());
      groups.entry(key).or_default().push(d);
    }
  }

  let total_errors: usize = groups
    .values()
    .map(|ds| ds.iter().filter(|d| d.severity == Severity::Error).count())
    .sum();
  let total_warnings = groups.values().map(|ds| ds.len()).sum::<usize>() - total_errors;

  match args.format {
    Format::Simple => render_simple(&groups),
    Format::Crisp => {
      let color = color_enabled(std::io::stdout().is_terminal());
      let n = groups.len();
      for (i, (file, diags)) in groups.iter().enumerate() {
        let source = std::fs::read_to_string(file).ok();
        let block = vass_core::error::crisp::render_file_block(
          file,
          diags,
          source.as_deref(),
          Some((i + 1, n)),
          color,
        );
        println!("{block}");
      }
      print!(
        "{}",
        vass_core::error::crisp::render_summary(
          total_errors,
          total_warnings,
          n,
          paths.len(),
          color
        )
      );
    }
  }

  if total_errors > 0 {
    std::process::exit(1);
  }
  Ok(())
}

/// Legacy one-line-per-file rendering (piped-CI friendly, no ANSI)
fn render_simple(groups: &BTreeMap<String, Vec<Diagnostic>>) {
  let checked = groups.len();
  for (file, diags) in groups {
    println!("ERROR   {file}");
    for d in diags {
      for line in format!("{d}").lines() {
        println!("        {line}");
      }
    }
  }
  if checked == 0 {
    eprintln!("all files OK");
  }
}

/// Collect every diagnostic for one file. The balance scan runs first; if
/// it found bracket problems the parse is skipped (they are the root
/// cause — a cascade of parser errors on top would only bury them).
fn check_file(path: &Path, parse_only: bool) -> Vec<Diagnostic> {
  let mut out = Vec::new();
  let p = path.display().to_string();

  let source = match std::fs::read_to_string(path) {
    Ok(s) => s,
    Err(e) => {
      out.push(Diagnostic::from_error(
        &Error::io(format!("can't read `{p}`: {e}")).with_file(&p),
      ));
      return out;
    }
  };

  let fix = |mut e: Error| {
    if e.file.is_none() {
      e.file = Some(p.clone());
    }
    Diagnostic::from_error(&e)
  };

  let scan = vass_parser::scan_balance(&source);
  if scan.is_empty() {
    let syntax = vass_parser::detect_syntax(path);
    match vass_parser::parse(&source, syntax) {
      Err(e) => out.push(fix(e)),
      Ok(sheet) => {
        if !parse_only {
          let options = vass_evaluator::EvalOptions {
            entry: Some(path.to_path_buf()),
            load_paths: Vec::new(),
            loader: None,
          };
          if let Err(e) = vass_evaluator::evaluate_with(&sheet, options) {
            out.push(fix(e));
          }
        }
      }
    }
  } else {
    out.extend(scan.into_iter().map(fix));
  }

  out
}

/// Recursively collect `.vass`/`.vcss` files under `dir` (partials included
/// — they are compiled through `@use` and can carry errors of their own).
fn walk_dir(dir: &Path, out: &mut Vec<PathBuf>) -> anyhow::Result<()> {
  for e in std::fs::read_dir(dir)? {
    let p = e?.path();
    if p.is_dir() {
      // Skip dependency/vendor noise the same way VCSs do
      let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
      if matches!(name, "node_modules" | ".git" | "dist" | "target") {
        continue;
      }
      walk_dir(&p, out)?;
    } else if matches!(
      p.extension().and_then(|e| e.to_str()),
      Some("vass") | Some("vcss")
    ) {
      out.push(p);
    }
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn walk_skips_vendor_dirs() {
    let tmp = std::env::temp_dir().join("bass_check_walk_test");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(tmp.join("node_modules")).unwrap();
    std::fs::create_dir_all(tmp.join("src")).unwrap();
    std::fs::write(tmp.join("node_modules/skip.vcss"), ".a{}").unwrap();
    std::fs::write(tmp.join("src/keep.vcss"), ".a{}").unwrap();
    std::fs::write(tmp.join("top.vass"), ".a{}").unwrap();
    std::fs::write(tmp.join("ignored.txt"), "nope").unwrap();
    let mut found = Vec::new();
    walk_dir(&tmp, &mut found).unwrap();
    let mut names: Vec<String> = found
      .iter()
      .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
      .collect();
    names.sort();
    assert_eq!(names, vec!["keep.vcss", "top.vass"]);
    let _ = std::fs::remove_dir_all(&tmp);
  }
}
