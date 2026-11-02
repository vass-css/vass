//! CLI subprocess integration tests.
//!
//! Runs the `vass` binary as a subprocess to exercise command paths that
//! call `std::process::exit` (missing files, empty input, errors) and cannot
//! be asserted from an in-process unit test without killing the test binary.

use std::process::Command;

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
