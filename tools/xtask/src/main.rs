//! VASS development automation.
//!
//! All project automation lives here. Pure Rust — no shell, no Python.
//!
//! Usage: `cargo xtask <command> [args]`

use clap::{Parser, Subcommand};
use std::process::ExitCode;

mod commands;

#[derive(Parser, Debug)]
#[command(name = "xtask", about = "VASS development automation")]
struct Cli {
  #[command(subcommand)]
  command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
  /// Run all quality gates: fmt, clippy, check, test
  Audit,
  /// Run cargo bench
  Bench {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
  },
  /// Run cargo check
  Check {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
  },
  /// Run cargo clean
  Clean {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
  },
  /// Generate shell completions
  Completions {
    #[arg(value_enum)]
    shell: clap_complete::Shell,
  },
  /// Run cargo fmt --check
  Fmt,
  /// Run cargo clippy
  Lint {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
  },
  /// Package crates for publishing
  Package {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
  },
  /// Validate a release version
  Release { version: String },
  /// Run cargo test
  Test {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
  },
  /// Run all verification gates
  Verify,
  /// Print the workspace version
  Version,
}

fn main() -> ExitCode {
  let cli = Cli::parse();
  let result = match cli.command {
    Commands::Audit => commands::audit::run(),
    Commands::Bench { args } => commands::bench::run(&args),
    Commands::Check { args } => commands::check::run(&args),
    Commands::Clean { args } => commands::clean::run(&args),
    Commands::Completions { shell } => commands::completions::run(shell),
    Commands::Fmt => commands::fmt::run(),
    Commands::Lint { args } => commands::lint::run(&args),
    Commands::Package { args } => commands::package::run(&args),
    Commands::Release { version } => commands::release::run(&version),
    Commands::Test { args } => commands::test::run(&args),
    Commands::Verify => commands::verify::run(),
    Commands::Version => commands::version::run(),
  };

  match result {
    Ok(()) => ExitCode::SUCCESS,
    Err(e) => {
      eprintln!("xtask: {e:#}");
      ExitCode::FAILURE
    }
  }
}
