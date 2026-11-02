//! Subprocess tests for the VASS LSP server entry point.

use std::io::Write;
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

fn frame(message: &str) -> String {
  format!("Content-Length: {}\\r\\n\\r\\n{message}", message.len())
}

#[test]
fn no_arguments_starts_stdio_server_and_handles_shutdown() {
  let mut child = Command::new(env!("CARGO_BIN_EXE_vass-lsp-server"))
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .expect("LSP server executable should start");

  let input = [
    frame(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"processId":null,"rootUri":null,"capabilities":{}}}"#),
    frame(r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#),
    frame(r#"{"jsonrpc":"2.0","id":2,"method":"shutdown","params":{}}"#),
    frame(r#"{"jsonrpc":"2.0","method":"exit","params":null}"#),
  ]
  .concat();

  child
    .stdin
    .take()
    .expect("stdin should be piped")
    .write_all(input.as_bytes())
    .expect("LSP protocol messages should be sent");

  let output = child.wait_with_output().expect("LSP server should exit");
  assert!(
    output.status.success(),
    "LSP server should shut down cleanly; stderr: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  assert!(
    String::from_utf8_lossy(&output.stdout).contains("Content-Length:"),
    "LSP responses should use Content-Length framing"
  );
}
