//! `vass fmt` — format `.vass`/`.vcss` source files.
//!
//! Prints the formatted source to stdout by default; `--write` rewrites
//! files in place; `--check` exits 1 when any file would be reformatted
//! (rustfmt-style, CI friendly).

use std::path::{Path, PathBuf};

use clap::Args as ClapArgs;

/// Format VASS source files
#[derive(ClapArgs, Debug)]
pub struct Args {
  /// Input files (use `-` to read stdin)
  #[arg(value_name = "FILES")]
  pub files: Vec<String>,

  /// Rewrite files in place instead of printing
  #[arg(long, short)]
  pub write: bool,

  /// Exit with 1 if files are not formatted (no output written)
  #[arg(long, conflicts_with = "write")]
  pub check: bool,
}

/// Run the fmt command
pub async fn run(args: Args) -> anyhow::Result<()> {
  if args.files.is_empty() {
    anyhow::bail!("no input files (pass files, or `-` for stdin)");
  }

  let stdin_dash = args.files.iter().any(|f| f == "-");
  if stdin_dash && args.files.len() > 1 {
    anyhow::bail!("`-` (stdin) cannot be combined with other files");
  }

  let mut changed = 0usize;
  let mut checked = 0usize;

  for file in &args.files {
    if file == "-" {
      let mut src = String::new();
      std::io::Read::read_to_string(&mut std::io::stdin(), &mut src)?;
      let braces = guess_braces(&src);
      print!("{}", vass_lsp::format::format_source(&src, braces));
      checked += 1;
      continue;
    }

    let path = PathBuf::from(file);
    let source = std::fs::read_to_string(&path)
      .map_err(|e| anyhow::anyhow!("can't read `{}`: {}", path.display(), e))?;
    let braces = path_braces(&path, &source);
    let formatted = vass_lsp::format::format_source(&source, braces);
    checked += 1;

    if args.check {
      if formatted != source {
        println!("{}: would be reformatted", path.display());
        changed += 1;
      }
    } else if args.write {
      if formatted != source {
        std::fs::write(&path, &formatted)?;
        eprintln!("{}: formatted", path.display());
        changed += 1;
      }
    } else {
      print!("{formatted}");
    }
  }

  if args.check {
    if changed > 0 {
      anyhow::bail!("{changed} of {checked} file(s) not formatted");
    }
    println!("{checked} file(s) formatted OK");
  }
  Ok(())
}

/// Braces syntax is declared by the `.vcss` extension; `.sass` is indented;
/// anything else (or stdin) falls back to sniffing for an opening brace.
fn path_braces(path: &Path, source: &str) -> bool {
  match path.extension().and_then(|e| e.to_str()) {
    Some("vcss") | Some("scss") => true,
    Some("vass") | Some("sass") => false,
    _ => guess_braces(source),
  }
}

fn guess_braces(source: &str) -> bool {
  source.contains('{')
}
