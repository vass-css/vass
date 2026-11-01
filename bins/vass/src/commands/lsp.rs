//! `vass lsp` — start the Language Server over stdio.
//!
//! Equivalent to running the standalone `vass-lsp-server` binary; embedded
//! here so a single `vass` installation is enough for editor integration.

use std::io::IsTerminal;

use clap::Args as ClapArgs;

/// Start the LSP server (stdio JSON-RPC)
#[derive(ClapArgs, Debug)]
pub struct Args {
  /// Transport to use (only `stdio` is supported)
  #[arg(long, default_value = "stdio")]
  pub transport: String,

  /// Shorthand for `--transport stdio`.
  ///
  /// Many editor integrations (VS Code, Neovim, Emacs LSP clients) launch
  /// the server as `<server> --stdio` by convention, so the flag must be
  /// accepted even though stdio is already the default transport.
  #[arg(long, hide = true)]
  pub stdio: bool,
}

/// Run the lsp command
pub async fn run(args: Args) -> anyhow::Result<()> {
  if args.transport != "stdio" {
    anyhow::bail!(
      "unsupported transport `{}` (only `stdio` is supported; `--stdio` is accepted as a shorthand)",
      args.transport
    );
  }

  // Logging is already initialized by `main` with a stderr writer; stdout
  // is reserved for LSP messages.
  tracing::info!(
    "VASS LSP server {} starting (stdio transport)",
    env!("CARGO_PKG_VERSION")
  );

  // Friendly hint for interactive terminals: editors launch the server
  // with piped stdio, so a TTY on stdin means a human is waiting — explain
  // why nothing appears to happen. Never printed when an editor starts it.
  if std::io::stdin().is_terminal() {
    eprintln!(
      "VASS LSP server {} — listening on stdio (JSON-RPC)",
      env!("CARGO_PKG_VERSION")
    );
    eprintln!("This command speaks the Language Server Protocol, so it");
    eprintln!("shows no human-readable output in a terminal by design.");
    eprintln!("Connect an editor instead:");
    eprintln!("  * VS Code: install the VASS extension and open a .vass/.vcss file");
    eprintln!("  * Other editors: point the LSP client at `vass lsp` (stdio)");
    eprintln!("Press Ctrl+C to stop.");
  }

  // `server.run()` performs blocking stdio reads/writes. Run it on
  // `spawn_blocking` so the Tokio runtime is not blocked.
  let code = tokio::task::spawn_blocking(vass_lsp::run_stdio)
    .await
    .map_err(|e| anyhow::anyhow!("LSP server task failed: {e}"))?;

  tracing::info!("VASS LSP server exiting with code {code}");
  if code != 0 {
    anyhow::bail!("LSP server exited with code {code}");
  }
  Ok(())
}
