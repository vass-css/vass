//! Subprocess tests for the VASS LSP server entry point.

use std::process::{Command, Stdio};

fn run_server(args: &[&str]) -> std::process::Output {
  Command::new(env!("CARGO_BIN_EXE_vass-lsp-server"))
    .args(args)
    .stdin(Stdio::null())
    .output()
    .expect("LSP server executable should start")
}

#[test]
fn version_long_flag_prints_package_version() {
  let output = run_server(&["--version"]);
  assert!(output.status.success());
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(stdout.contains(env!("CARGO_PKG_VERSION")));
  assert!(stdout.starts_with("vass-lsp-server "));
}

#[test]
fn version_short_flag_prints_package_version() {
  let output = run_server(&["-V"]);
  assert!(output.status.success());
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(stdout.contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn unknown_argument_prints_usage_and_exits_two() {
  let output = run_server(&["--not-a-real-flag"]);
  assert_eq!(output.status.code(), Some(2));

  let stderr = String::from_utf8_lossy(&output.stderr);
  assert!(stderr.contains("unknown argument"));
  assert!(stderr.contains("usage: vass-lsp-server [--version]"));
  assert!(stderr.contains("communicates over stdio"));
}

#[test]
fn no_arguments_starts_stdio_server_and_exits_on_eof() {
  let output = run_server(&[]);
  assert!(
    output.status.success(),
    "LSP server should shut down cleanly on stdin EOF; stderr: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  assert!(output.stdout.is_empty(), "LSP protocol output must not be polluted");
}
