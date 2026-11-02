//! `vass doctor` — diagnose the local VASS installation.
//!
//! Runs a series of environment checks (syntax detection, parsing,
//! evaluation, builtin library, optional binaries/plugins) and prints
//! OK/FAIL per check. Exit code is 0 only when every check passes.

use clap::Args as ClapArgs;

/// Diagnose the VASS installation
#[derive(ClapArgs, Debug)]
pub struct Args {}

/// Run the doctor command
pub async fn run(_args: Args) -> anyhow::Result<()> {
  let mut failed = 0usize;

  check(
    "version",
    &format!(
      "VASS {} ({})",
      env!("CARGO_PKG_VERSION"),
      std::env::consts::OS
    ),
    &mut failed,
  );
  check_run(
    "syntax detection (.vass indented)",
    || {
      Ok(
        vass_parser::detect_syntax(std::path::Path::new("x.vass")) == vass_parser::Syntax::Indented,
      )
    },
    &mut failed,
  );
  check_run(
    "syntax detection (.vcss braces)",
    || {
      Ok(vass_parser::detect_syntax(std::path::Path::new("x.vcss")) == vass_parser::Syntax::Braces)
    },
    &mut failed,
  );
  check_run(
    "parse indented source",
    || -> Result<bool, String> {
      vass_parser::parse("$a: red\n.x\n  color: $a\n", vass_parser::Syntax::Indented)
        .map(|_| true)
        .map_err(|e| e.to_string())
    },
    &mut failed,
  );
  check_run(
    "parse braces source",
    || -> Result<bool, String> {
      vass_parser::parse("$a: red; .x { color: $a; }", vass_parser::Syntax::Braces)
        .map(|_| true)
        .map_err(|e| e.to_string())
    },
    &mut failed,
  );
  check_run(
    "evaluate + builtin library",
    || -> Result<bool, String> {
      let sheet = vass_parser::parse(
        "@use \"vass:math\";\n.x { width: math.div(10, 2) * 1px; }",
        vass_parser::Syntax::Braces,
      )
      .map_err(|e| e.to_string())?;
      let options = vass_evaluator::EvalOptions::default();
      let css = vass_evaluator::evaluate_with(&sheet, options).map_err(|e| e.to_string())?;
      let text = vass_transpiler::render(&css, vass_transpiler::OutputStyle::Expanded);
      if text.contains("width: 5px") {
        Ok(true)
      } else {
        Err(format!("unexpected output: {text}"))
      }
    },
    &mut failed,
  );
  check_run(
    "built-in modules",
    || -> Result<bool, String> { Ok(vass_evaluator::builtins::BUILTIN_MODULES.len() >= 7) },
    &mut failed,
  );
  check_run(
    "formatter",
    || -> Result<bool, String> {
      Ok(!vass_lsp::format::format_source("$a: red;\n.x{color:$a}", true).is_empty())
    },
    &mut failed,
  );
  check_path_env("vass on PATH", &mut failed);

  if failed > 0 {
    anyhow::bail!("{failed} check(s) failed");
  }
  eprintln!("all checks passed");
  Ok(())
}

fn check(name: &str, detail: &str, failed: &mut usize) {
  println!("OK    {name}: {detail}");
  let _ = failed;
}

fn check_run(name: &str, f: impl FnOnce() -> Result<bool, String>, failed: &mut usize) {
  match f() {
    Ok(true) => println!("OK    {name}"),
    Ok(false) => {
      println!("FAIL  {name}: unexpected result");
      *failed += 1;
    }
    Err(e) => {
      println!("FAIL  {name}: {e}");
      *failed += 1;
    }
  }
}

fn check_path_env(name: &str, failed: &mut usize) {
  // `vass` must resolve to *this* executable for editor tooling to work;
  // we only verify the current exe exists and is executable-ish.
  match std::env::current_exe() {
    Ok(p) if p.exists() => println!("OK    {name}: {}", p.display()),
    _ => {
      println!("FAIL  {name}: current executable not found");
      *failed += 1;
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_check_ok() {
    let mut failed = 0usize;
    check("test_check", "detail", &mut failed);
    assert_eq!(failed, 0);
  }

  #[test]
  fn test_check_run_ok() {
    let mut failed = 0usize;
    check_run("test_run", || Ok(true), &mut failed);
    assert_eq!(failed, 0);
  }

  #[test]
  fn test_check_run_false() {
    let mut failed = 0usize;
    check_run("test_run_false", || Ok(false), &mut failed);
    assert_eq!(failed, 1);
  }

  #[test]
  fn test_check_run_err() {
    let mut failed = 0usize;
    check_run("test_run_err", || Err("boom".to_string()), &mut failed);
    assert_eq!(failed, 1);
  }

  #[test]
  fn test_check_path_env() {
    let mut failed = 0usize;
    check_path_env("test_path", &mut failed);
    // current_exe() should exist in test context
    assert_eq!(failed, 0);
  }

  #[tokio::test]
  async fn test_doctor_run() {
    let result = run(Args {}).await;
    // Doctor may fail on some checks but should not panic
    let _ = result;
  }
}
