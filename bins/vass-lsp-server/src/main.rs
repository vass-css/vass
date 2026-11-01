//! # VASS LSP Server
//!
//! Language Server Protocol server for the VASS CSS preprocessor.
//!
//! Speaks JSON-RPC 2.0 over stdio (`Content-Length` framing). Start it from
//! an editor/IDE client; it publishes diagnostics on open/change/close and
//! serves completion, hover, definition, document symbols and formatting.
//!
//! Flags:
//! - `--version` / `-V`: print version and exit

fn main() -> std::process::ExitCode {
  let mut args = std::env::args().skip(1);
  if let Some(arg) = args.next() {
    if arg == "--version" || arg == "-V" {
      println!("vass-lsp-server {}", env!("CARGO_PKG_VERSION"));
      return std::process::ExitCode::SUCCESS;
    }
    eprintln!("unknown argument: {arg}");
    eprintln!("usage: vass-lsp-server [--version]");
    eprintln!("This server is started by an LSP client (editor/IDE) and communicates over stdio.");
    return std::process::ExitCode::from(2);
  }

  // All logging goes to stderr; stdout is reserved for LSP messages.
  tracing_subscriber::fmt()
    .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
    .with_writer(std::io::stderr)
    .with_target(false)
    .init();

  tracing::info!(
    "VASS LSP server {} starting (stdio transport)",
    env!("CARGO_PKG_VERSION")
  );

  let code = vass_lsp::run_stdio();
  tracing::info!("VASS LSP server exiting with code {code}");
  std::process::ExitCode::from(code as u8)
}
