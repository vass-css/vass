//! # VASS LSP
//!
//! Language Server Protocol implementation for the VASS CSS preprocessor.
//!
//! ## Features
//!
//! - Parse + evaluate diagnostics with precise ranges (`textDocument/publishDiagnostics`)
//! - Context-aware completion: variables, mixins, functions, at-rules,
//!   built-in modules and their members
//! - Hover with signatures and documentation for local symbols and built-ins
//! - Go-to-definition for variables, mixins and functions
//! - Document outline (`textDocument/documentSymbol`)
//! - Conservative document formatting for both `.vass` and `.vcss`
//! - JSON-RPC 2.0 over stdio with `Content-Length` framing
//!
//! ## Example
//!
//! ```no_run
//! use vass_lsp::server::Server;
//! let mut server = Server::new();
//! let mut stdin = std::io::stdin().lock();
//! let mut stdout = std::io::stdout().lock();
//! let code = server.run(&mut stdin, &mut stdout);
//! std::process::exit(code);
//! ```

#![warn(missing_docs)]
#![allow(dead_code)]

pub mod analysis;
pub mod completion;
pub mod diag;
pub mod format;
pub mod rpc;
pub mod server;

pub use server::run_stdio;

/// Crate result type alias
pub type Result<T> = std::result::Result<T, Error>;

/// LSP errors
#[derive(Debug, thiserror::Error)]
pub enum Error {
  /// LSP error
  #[error("LSP error: {0}")]
  Lsp(String),

  /// Parse error
  #[error("Parse error: {0}")]
  Parse(#[from] serde_json::Error),
}
