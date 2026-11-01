//! `vass version` — print detailed version information.

use clap::Args as ClapArgs;

/// Print detailed version information
#[derive(ClapArgs, Debug)]
pub struct Args {}

/// Run the version command
pub async fn run(_args: Args) -> anyhow::Result<()> {
  println!("vass {}", env!("CARGO_PKG_VERSION"));
  println!(
    "platform: {}/{}",
    std::env::consts::OS,
    std::env::consts::ARCH
  );
  println!(
    "profile:  {}",
    if cfg!(debug_assertions) {
      "debug"
    } else {
      "release"
    }
  );
  println!(
    "modules:  {}",
    vass_evaluator::builtins::BUILTIN_MODULES.join(", ")
  );
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_version_run() {
    let result = run(Args {});
    assert!(result.is_ok());
  }
}
