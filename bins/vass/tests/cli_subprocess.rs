//! CLI subprocess integration tests.
//!
//! Runs the `vass` binary as a subprocess to exercise command paths that
//! call `std::process::exit` (missing files, empty input, errors) and cannot
//! be asserted from an in-process unit test without killing the test binary.

use std::io::Write;
use std::process::{Command, Stdio};

fn vass_bin() -> String {
  env!("CARGO_BIN_EXE_vass").to_string()
}

fn run_vass(args: &[&str]) -> std::process::Output {
  Command::new(vass_bin())
    .args(args)
    .output()
    .expect("vass binary should run")
}

#[test]
fn cli_no_args_exits_2() {
  let out = run_vass(&[]);
  assert!(!out.status.success());
  assert_eq!(out.status.code(), Some(2));
}

#[test]
fn cli_unknown_command_exits_nonzero() {
  let out = run_vass(&["__no_such_command__"]);
  assert!(!out.status.success());
}

#[test]
fn check_nonexistent_file_exits_nonzero() {
  let out = run_vass(&["check", "nonexistent_file_for_subprocess_test.vass"]);
  assert!(!out.status.success());
  let stderr = String::from_utf8_lossy(&out.stderr).to_string();
  assert!(stderr.contains("does not exist"), "got: {stderr}");
}

#[test]
fn check_empty_paths_exits_nonzero() {
  let out = run_vass(&["check"]);
  assert!(!out.status.success());
  let stderr = String::from_utf8_lossy(&out.stderr).to_string();
  assert!(
    stderr.contains("no `.vass`/`.vcss` files found") || stderr.contains("required"),
    "got: {stderr}"
  );
}

#[test]
fn check_error_file_exits_1() {
  let dir = std::env::temp_dir().join(format!("vass-sub-{}", std::process::id()));
  std::fs::create_dir_all(&dir).unwrap();
  let path = dir.join("bad.vass");
  std::fs::write(&path, ".a { color: red\n").unwrap();
  let out = run_vass(&["check", path.to_str().unwrap()]);
  assert!(!out.status.success());
  assert_eq!(out.status.code(), Some(1));
  let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn check_ok_file_exits_0() {
  let dir = std::env::temp_dir().join(format!("vass-sub-ok-{}", std::process::id()));
  std::fs::create_dir_all(&dir).unwrap();
  let path = dir.join("ok.vass");
  std::fs::write(&path, ".a\n  color: red\n").unwrap();
  let out = run_vass(&["check", path.to_str().unwrap()]);
  assert!(
    out.status.success(),
    "stderr: {}",
    String::from_utf8_lossy(&out.stderr)
  );
  let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn fmt_no_files_exits_nonzero() {
  let out = run_vass(&["fmt"]);
  assert!(!out.status.success());
}

#[test]
fn fmt_nonexistent_file_exits_nonzero() {
  let out = run_vass(&["fmt", "nonexistent_file_for_subprocess_test.vass"]);
  assert!(!out.status.success());
  let stderr = String::from_utf8_lossy(&out.stderr).to_string();
  assert!(stderr.contains("can't read"), "got: {stderr}");
}

#[test]
fn build_nonexistent_dir_exits_nonzero() {
  let out = run_vass(&["build", ".nonexistent_dir_for_subprocess_test"]);
  assert!(!out.status.success());
  let stderr = String::from_utf8_lossy(&out.stderr).to_string();
  assert!(stderr.contains("is not a directory"), "got: {stderr}");
}

#[test]
fn build_no_entries_exits_nonzero() {
  let dir = std::env::temp_dir().join(format!("vass-sub-build-{}", std::process::id()));
  std::fs::create_dir_all(&dir).unwrap();
  std::fs::write(dir.join("_partial.vass"), "$x: 1\n").unwrap();
  let out = run_vass(&["build", dir.to_str().unwrap()]);
  assert!(!out.status.success());
  let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn compile_error_exits_1() {
  let dir = std::env::temp_dir().join(format!("vass-sub-compile-{}", std::process::id()));
  std::fs::create_dir_all(&dir).unwrap();
  let path = dir.join("bad.vass");
  std::fs::write(&path, "@error \"boom\"\n").unwrap();
  let out = run_vass(&["compile", path.to_str().unwrap()]);
  assert!(!out.status.success());
  assert_eq!(out.status.code(), Some(1));
  let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn compile_ok_exits_0() {
  let dir = std::env::temp_dir().join(format!("vass-sub-compileok-{}", std::process::id()));
  std::fs::create_dir_all(&dir).unwrap();
  let path = dir.join("ok.vass");
  std::fs::write(&path, ".a\n  color: red\n").unwrap();
  let out = run_vass(&["compile", path.to_str().unwrap()]);
  assert!(
    out.status.success(),
    "stderr: {}",
    String::from_utf8_lossy(&out.stderr)
  );
  let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn lsp_unsupported_transport_exits_nonzero() {
  let out = run_vass(&["lsp", "--transport", "tcp"]);
  assert!(!out.status.success());
}

#[test]
fn version_exits_0() {
  let out = run_vass(&["version"]);
  assert!(out.status.success());
  let stdout = String::from_utf8_lossy(&out.stdout).to_string();
  assert!(!stdout.is_empty());
}

#[test]
fn exec_ok_exits_0() {
  let out = run_vass(&["exec", ".a { color: red; }"]);
  assert!(
    out.status.success(),
    "stderr: {}",
    String::from_utf8_lossy(&out.stderr)
  );
}

#[test]
fn explain_ok_exits_0() {
  let out = run_vass(&["explain", "E0001"]);
  assert!(out.status.success());
}

#[test]
fn list_ok_exits_0() {
  let out = run_vass(&["list"]);
  assert!(out.status.success());
}

#[test]
fn stats_ok_exits_0() {
  let dir = std::env::temp_dir().join(format!("vass-sub-stats-{}", std::process::id()));
  std::fs::create_dir_all(&dir).unwrap();
  let path = dir.join("ok.vass");
  std::fs::write(&path, ".a\n  color: red\n").unwrap();
  let out = run_vass(&["stats", path.to_str().unwrap()]);
  assert!(
    out.status.success(),
    "stderr: {}",
    String::from_utf8_lossy(&out.stderr)
  );
  let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn doctor_ok_exits_0() {
  let out = run_vass(&["doctor"]);
  assert!(out.status.success());
}

#[test]
fn plugin_subcommand_help_exits_0() {
  let out = run_vass(&["plugin", "--help"]);
  assert!(out.status.success());
}

#[test]
fn completions_ok_exits_0() {
  let out = run_vass(&["completions", "bash"]);
  assert!(out.status.success());
}


fn run_vass_with_stdin(args: &[&str], input: &str) -> std::process::Output {
  let mut child = Command::new(vass_bin())
    .args(args)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .expect("vass binary should run");

  child
    .stdin
    .take()
    .expect("stdin should be piped")
    .write_all(input.as_bytes())
    .expect("input should be sent to vass");

  child.wait_with_output().expect("vass process should exit")
}

fn unique_test_dir(label: &str) -> std::path::PathBuf {
  let dir = std::env::temp_dir().join(format!(
    "vass-sub-{label}-{}-{}",
    std::process::id(),
    std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .expect("system clock")
      .as_nanos()
  ));
  std::fs::create_dir_all(&dir).expect("test directory should be created");
  dir
}

fn lsp_frame(message: &str) -> String {
  format!("Content-Length: {}\r\n\r\n{message}", message.len())
}

#[test]
fn repl_placeholder_runs_and_exits_successfully() {
  let out = run_vass(&["repl"]);
  assert!(
    out.status.success(),
    "stderr: {}",
    String::from_utf8_lossy(&out.stderr)
  );
  let stdout = String::from_utf8_lossy(&out.stdout);
  assert!(stdout.contains("VASS REPL"));
  assert!(stdout.contains("not yet implemented"));
}

#[test]
fn lsp_stdio_session_initializes_and_shuts_down() {
  let input = [
    lsp_frame(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"processId":null,"rootUri":null,"capabilities":{}}}"#),
    lsp_frame(r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#),
    lsp_frame(r#"{"jsonrpc":"2.0","id":2,"method":"shutdown","params":{}}"#),
    lsp_frame(r#"{"jsonrpc":"2.0","method":"exit","params":null}"#),
  ]
  .concat();

  let out = run_vass_with_stdin(&["lsp", "--stdio"], &input);
  assert!(
    out.status.success(),
    "stderr: {}",
    String::from_utf8_lossy(&out.stderr)
  );
  assert!(
    String::from_utf8_lossy(&out.stdout).contains("Content-Length:"),
    "LSP responses should use Content-Length framing"
  );
}

#[test]
fn compile_source_map_reports_unsupported_feature() {
  let dir = unique_test_dir("source-map");
  let path = dir.join("main.vcss");
  std::fs::write(&path, ".a { color: red; }\n").expect("fixture should be written");

  let out = run_vass(&["compile", "--source-map", path.to_str().unwrap()]);
  assert!(!out.status.success());
  assert!(
    String::from_utf8_lossy(&out.stderr).contains("--source-map is not supported yet"),
    "stderr: {}",
    String::from_utf8_lossy(&out.stderr)
  );
  let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn compile_minify_uses_compressed_output() {
  let dir = unique_test_dir("minify");
  let path = dir.join("main.vcss");
  std::fs::write(&path, ".a { color: red; }\n").expect("fixture should be written");

  let out = run_vass(&["compile", "--minify", path.to_str().unwrap()]);
  assert!(
    out.status.success(),
    "stderr: {}",
    String::from_utf8_lossy(&out.stderr)
  );
  let stdout = String::from_utf8_lossy(&out.stdout);
  assert!(stdout.contains(".a{"), "unexpected output: {stdout}");
  let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn fmt_stdin_formats_braces_syntax() {
  let out = run_vass_with_stdin(&["fmt", "-"], ".a{color:red}\n");
  assert!(
    out.status.success(),
    "stderr: {}",
    String::from_utf8_lossy(&out.stderr)
  );
  let stdout = String::from_utf8_lossy(&out.stdout);
  assert!(stdout.contains("color"), "unexpected output: {stdout}");
  assert_ne!(stdout, ".a{color:red}\n");
}

#[test]
fn fmt_write_then_check_keeps_formatted_file_unchanged() {
  let dir = unique_test_dir("fmt-write");
  let path = dir.join("main.vcss");
  std::fs::write(&path, ".a{color:red}\n").expect("fixture should be written");

  let write = run_vass(&["fmt", "--write", path.to_str().unwrap()]);
  assert!(
    write.status.success(),
    "stderr: {}",
    String::from_utf8_lossy(&write.stderr)
  );
  let formatted = std::fs::read_to_string(&path).expect("formatted file should exist");
  assert_ne!(formatted, ".a{color:red}\n");

  let check = run_vass(&["fmt", "--check", path.to_str().unwrap()]);
  assert!(
    check.status.success(),
    "stderr: {}",
    String::from_utf8_lossy(&check.stderr)
  );
  let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn fmt_rejects_stdin_combined_with_files() {
  let out = run_vass_with_stdin(&["fmt", "-", "another.vass"], ".a {}\n");
  assert!(!out.status.success());
  assert!(
    String::from_utf8_lossy(&out.stderr).contains("cannot be combined"),
    "stderr: {}",
    String::from_utf8_lossy(&out.stderr)
  );
}

#[test]
fn build_success_writes_only_entry_stylesheets() {
  let dir = unique_test_dir("build-success");
  let source = dir.join("src");
  let nested = source.join("nested");
  let output = dir.join("dist");
  std::fs::create_dir_all(&nested).expect("nested source directory");
  std::fs::write(source.join("main.vass"), ".main\n  color: red\n")
    .expect("entry stylesheet");
  std::fs::write(source.join("_partial.vass"), "$accent: blue\n")
    .expect("partial stylesheet");
  std::fs::write(nested.join("nested.vcss"), ".nested { color: blue; }\n")
    .expect("nested entry stylesheet");

  let out = run_vass(&[
    "build",
    source.to_str().unwrap(),
    "--out-dir",
    output.to_str().unwrap(),
  ]);
  assert!(
    out.status.success(),
    "stderr: {}",
    String::from_utf8_lossy(&out.stderr)
  );
  assert!(output.join("main.css").is_file());
  assert!(output.join("nested/nested.css").is_file());
  assert!(!output.join("_partial.css").exists());
  let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn build_rejects_invalid_output_style_after_finding_entries() {
  let dir = unique_test_dir("build-invalid-style");
  let path = dir.join("main.vass");
  std::fs::write(&path, ".main\n  color: red\n").expect("fixture should be written");

  let out = run_vass(&[
    "build",
    dir.to_str().unwrap(),
    "--style",
    "invalid",
  ]);
  assert!(!out.status.success());
  assert!(
    String::from_utf8_lossy(&out.stderr).contains("invalid output style"),
    "stderr: {}",
    String::from_utf8_lossy(&out.stderr)
  );
  let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn lint_alias_checks_valid_file() {
  let dir = unique_test_dir("lint");
  let path = dir.join("main.vass");
  std::fs::write(&path, ".main\n  color: red\n").expect("fixture should be written");

  let out = run_vass(&["lint", path.to_str().unwrap()]);
  assert!(
    out.status.success(),
    "stderr: {}",
    String::from_utf8_lossy(&out.stderr)
  );
  let _ = std::fs::remove_dir_all(&dir);
}
