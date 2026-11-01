//! # VASS CLI
//!
//! Command-line interface for the VASS CSS preprocessor.

use clap::{CommandFactory, Parser};
use tracing_subscriber::EnvFilter;

mod commands;

/// VASS - A modern CSS preprocessor
#[derive(Parser, Debug)]
#[command(name = "vass")]
#[command(author, version, about, long_about = None)]
struct Cli {
  #[command(subcommand)]
  command: Commands,
}

/// Available commands
#[derive(Parser, Debug)]
enum Commands {
  /// Compile a VASS file to CSS
  Compile(commands::compile::Args),
  /// Compile a VASS file to CSS (alias of `compile`)
  Run(commands::compile::Args),
  /// Build all stylesheets in a directory
  Build(commands::build::Args),
  /// Check files for errors without writing output
  Check(commands::check::Args),
  /// Lint files for errors (alias of `check`)
  Lint(commands::lint::Args),
  /// Format VASS source files
  Fmt(commands::fmt::Args),
  /// Evaluate inline VASS code
  Exec(commands::exec::Args),
  /// Explain a diagnostic code (E0001..E0012)
  Explain(commands::explain::Args),
  /// List built-in modules, functions and variables
  List(commands::list::Args),
  /// Print compile statistics for a file
  Stats(commands::stats::Args),
  /// Watch files for changes and recompile
  Watch(commands::watch::Args),
  /// Start the LSP server (stdio JSON-RPC)
  Lsp(commands::lsp::Args),
  /// Start an interactive REPL
  Repl(commands::repl::Args),
  /// Initialize a new VASS project
  Init(commands::init::Args),
  /// Manage plugins
  Plugin(commands::plugin::Args),
  /// Diagnose the VASS installation
  Doctor(commands::doctor::Args),
  /// Generate shell completions
  Completions(commands::completions::Args),
  /// Print detailed version information
  Version(commands::version::Args),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  // Exit silently on SIGPIPE (`vass ... | head`) like a well-behaved unix
  // CLI instead of panicking with "failed printing to stdout: Broken pipe".
  #[cfg(unix)]
  unsafe {
    libc::signal(libc::SIGPIPE, libc::SIG_DFL);
  }

  // Logging goes to stderr; stdout is reserved for command output (CSS,
  // formatted source, LSP frames, ...). `try_init` never panics if a
  // command installs its own subscriber.
  tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .with_writer(std::io::stderr)
    .with_target(false)
    .try_init()
    .ok();

  if std::env::args_os().len() == 1 {
    Cli::command().print_help()?;
    println!();
    std::process::exit(2);
  }

  let cli = Cli::parse();

  let result: anyhow::Result<()> = match cli.command {
    Commands::Compile(args) | Commands::Run(args) => commands::compile::run(args).await,
    Commands::Build(args) => commands::build::run(args).await,
    Commands::Check(args) => commands::check::run(args).await,
    Commands::Fmt(args) => commands::fmt::run(args).await,
    Commands::Lint(args) => commands::lint::run(args).await,
    Commands::Exec(args) => commands::exec::run(args).await,
    Commands::Explain(args) => commands::explain::run(args).await,
    Commands::List(args) => commands::list::run(args).await,
    Commands::Stats(args) => commands::stats::run(args).await,
    Commands::Watch(args) => commands::watch::run(args).await,
    Commands::Lsp(args) => commands::lsp::run(args).await,
    Commands::Repl(args) => commands::repl::run(args).await,
    Commands::Init(args) => commands::init::run(args).await,
    Commands::Plugin(args) => commands::plugin::run(args).await,
    Commands::Doctor(args) => commands::doctor::run(args).await,
    Commands::Completions(args) => commands::completions::run(args, Cli::command()),
    Commands::Version(args) => commands::version::run(args).await,
  };

  if let Err(e) = result {
    // Compile diagnostics render as ESLint-crisp blocks (rule, `🔥 N
    // errors 🔥` headline, source line, `↑` caret, `└─ ✖` message);
    // print the block verbatim instead of stacking anyhow's `Error:`
    // prefix on top of it.
    eprintln!("{e}");
    std::process::exit(1);
  }

  Ok(())
}
