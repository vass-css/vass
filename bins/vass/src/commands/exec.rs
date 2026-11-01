//! `vass exec` — evaluate inline stylesheet code.
//!
//! Takes code as an argument or reads `-` / `--stdin` from stdin; useful
//! for quick experiments, scripting and the test harness. The code syntax
//! defaults to the braces form (`.vcss`) for one-liners; `--indented`
//! selects the indented form.

use clap::Args as ClapArgs;

/// Evaluate inline VASS code
#[derive(ClapArgs, Debug)]
pub struct Args {
  /// VASS code to evaluate, or `-` to read from stdin
  #[arg(value_name = "CODE")]
  pub code: Option<String>,

  /// Read code from stdin
  #[arg(long)]
  pub stdin: bool,

  /// Treat the code as indented syntax (`.vass` form)
  #[arg(long)]
  pub indented: bool,

  /// Output style (expanded, compressed)
  #[arg(long, default_value = "expanded")]
  pub style: String,
}

/// Run the exec command
pub async fn run(args: Args) -> anyhow::Result<()> {
  let source = if args.stdin || args.code.as_deref() == Some("-") {
    let mut buf = String::new();
    std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)?;
    buf
  } else {
    args
      .code
      .clone()
      .ok_or_else(|| anyhow::anyhow!("no code given (pass CODE or use --stdin)"))?
  };

  let syntax = if args.indented {
    vass_parser::Syntax::Indented
  } else {
    vass_parser::Syntax::Braces
  };

  let sheet = vass_parser::parse(&source, syntax).map_err(|e| anyhow::anyhow!("{e}"))?;

  let options = vass_evaluator::EvalOptions {
    entry: None,
    load_paths: Vec::new(),
    loader: None,
  };
  let css = vass_evaluator::evaluate_with(&sheet, options).map_err(|e| anyhow::anyhow!("{e}"))?;

  let style = super::parse_style(&args.style)?;
  print!("{}", vass_transpiler::render(&css, style));
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_exec_run() {
    let result = run(Args {
      code: Some(".a { color: red; }".to_string()),
      stdin: false,
      indented: false,
      style: "expanded".to_string(),
    });
    assert!(result.is_ok());
  }

  #[test]
  fn test_exec_no_code() {
    let result = run(Args {
      code: None,
      stdin: false,
      indented: false,
      style: "expanded".to_string(),
    });
    assert!(result.is_err());
  }
}
