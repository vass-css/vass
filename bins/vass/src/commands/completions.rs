//! `vass completions` — generate shell completion scripts.
//!
//! Supported shells: bash, zsh, fish, powershell, elvish.
//! Usage example: `vass completions bash > /etc/bash_completion.d/vass`

use clap::Args as ClapArgs;
use clap_complete::{Shell, generate};

/// Generate shell completions
#[derive(ClapArgs, Debug)]
pub struct Args {
  /// Shell to generate completions for
  #[arg(value_enum, value_name = "SHELL")]
  pub shell: Shell,
}

/// Generate completions for the given shell against the CLI definition.
pub fn run(args: Args, cmd: clap::Command) -> anyhow::Result<()> {
  let mut cmd = cmd;
  let bin = cmd.get_name().to_string();
  generate(args.shell, &mut cmd, bin, &mut std::io::stdout());
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_completions_run() {
    let cmd = clap::Command::new("vass");
    let result = run(Args { shell: Shell::Bash }, cmd);
    assert!(result.is_ok());
  }
}
