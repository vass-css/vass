//! REPL command

use clap::Args as ClapArgs;

/// Start an interactive REPL
#[derive(ClapArgs, Debug)]
pub struct Args {
  /// Initial file to load
  #[arg(short, long)]
  pub file: Option<String>,
}

/// Run the REPL command
pub async fn run(_args: Args) -> anyhow::Result<()> {
  println!("VASS REPL v0.1.0");
  println!("Type :help for commands, :quit to exit");
  println!();

  // Placeholder - will be implemented in later phases
  println!("REPL not yet implemented. Check back in later phases.");

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_repl_args_exist() {
    let _ = Args { file: None };
    let _ = Args {
      file: Some("test.vass".to_string()),
    };
  }
}
