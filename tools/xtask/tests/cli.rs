//! End-to-end tests for the xtask command dispatcher.
//!
//! The fake Cargo executable records arguments without performing builds,
//! cleaning the real target directory, or publishing packages.

#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

struct TestContext {
  root: PathBuf,
  bin: PathBuf,
  log: PathBuf,
}

impl TestContext {
  fn new() -> Self {
    let nonce = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .expect("system clock should be after UNIX_EPOCH")
      .as_nanos();
    let root = std::env::temp_dir().join(format!("vass-xtask-{}-{nonce}", std::process::id()));
    let bin = root.join("bin");
    fs::create_dir_all(&bin).expect("temporary bin directory should be created");

    let cargo = bin.join("cargo");
    fs::write(
      &cargo,
      "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$VASS_XTASK_LOG\"\nexit \"$VASS_XTASK_EXIT\"\n",
    )
    .expect("fake Cargo executable should be written");
    let mut permissions = fs::metadata(&cargo)
      .expect("fake Cargo metadata should be available")
      .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&cargo, permissions)
      .expect("fake Cargo executable should be made executable");

    Self {
      log: root.join("cargo.log"),
      root,
      bin,
    }
  }

  fn run(&self, args: &[&str], cargo_exit: i32) -> Output {
    let original_path = std::env::var_os("PATH").unwrap_or_default();
    let path = std::iter::once(self.bin.clone())
      .chain(std::env::split_paths(&original_path))
      .collect::<Vec<_>>();
    let path = std::env::join_paths(path).expect("PATH should be constructible");

    Command::new(env!("CARGO_BIN_EXE_xtask"))
      .args(args)
      .current_dir(&self.root)
      .env("PATH", path)
      .env("VASS_XTASK_LOG", &self.log)
      .env("VASS_XTASK_EXIT", cargo_exit.to_string())
      .output()
      .expect("xtask executable should start")
  }

  fn run_without_cargo_in_path(&self, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_xtask"))
      .args(args)
      .current_dir(&self.root)
      .env("PATH", "")
      .env("VASS_XTASK_LOG", &self.log)
      .env("VASS_XTASK_EXIT", "0")
      .output()
      .expect("xtask executable should start")
  }

  fn calls(&self) -> Vec<String> {
    fs::read_to_string(&self.log)
      .unwrap_or_default()
      .lines()
      .map(str::to_owned)
      .collect()
  }

  fn clear_calls(&self) {
    let _ = fs::remove_file(&self.log);
  }
}

impl Drop for TestContext {
  fn drop(&mut self) {
    let _ = fs::remove_dir_all(&self.root);
  }
}

fn assert_success(output: &Output) {
  assert!(
    output.status.success(),
    "xtask should succeed; stderr: {}",
    String::from_utf8_lossy(&output.stderr)
  );
}

#[test]
fn dispatches_all_xtask_commands_and_forwards_cargo_arguments() {
  let ctx = TestContext::new();

  let output = ctx.run(&["audit"], 0);
  assert_success(&output);
  assert_eq!(
    ctx.calls(),
    vec![
      "fmt --all -- --check",
      "clippy --workspace --all-targets --locked -- -D warnings",
      "check --workspace --all-targets --locked",
      "test --workspace --locked",
      "doc --workspace --no-deps --locked",
    ]
  );

  ctx.clear_calls();
  assert_success(&ctx.run(&["bench", "--no-run"], 0));
  assert_eq!(ctx.calls(), vec!["bench --no-run"]);

  ctx.clear_calls();
  assert_success(&ctx.run(&["check", "-p", "vass-core"], 0));
  assert_eq!(ctx.calls(), vec!["check -p vass-core"]);

  ctx.clear_calls();
  fs::create_dir_all(ctx.root.join("completions")).expect("completions dir");
  fs::create_dir_all(ctx.root.join("docs")).expect("docs dir");
  fs::write(ctx.root.join("completions/vass.bash"), "generated").expect("bash completion");
  fs::write(ctx.root.join("completions/keep.txt"), "keep").expect("text file");
  fs::write(ctx.root.join("docs/generated.html"), "generated").expect("generated docs");
  fs::write(ctx.root.join("docs/keep.md"), "keep").expect("markdown docs");
  assert_success(&ctx.run(&["clean"], 0));
  assert_eq!(ctx.calls(), vec!["clean"]);
  assert!(!ctx.root.join("completions/vass.bash").exists());
  assert!(ctx.root.join("completions/keep.txt").exists());
  assert!(!ctx.root.join("docs/generated.html").exists());
  assert!(ctx.root.join("docs/keep.md").exists());

  ctx.clear_calls();
  assert_success(&ctx.run(&["fmt"], 0));
  assert_eq!(ctx.calls(), vec!["fmt --all -- --check"]);

  ctx.clear_calls();
  assert_success(&ctx.run(&["lint"], 0));
  assert_eq!(
    ctx.calls(),
    vec!["clippy --workspace --all-targets --locked -- -D warnings"]
  );

  ctx.clear_calls();
  assert_success(&ctx.run(&["package", "--list"], 0));
  assert_eq!(ctx.calls(), vec!["package --list"]);

  ctx.clear_calls();
  assert_success(&ctx.run(&["release", "v1.0.0"], 0));
  assert_eq!(
    ctx.calls(),
    vec![
      "fmt --all -- --check",
      "clippy --workspace --all-targets --locked -- -D warnings",
      "check --workspace --all-targets --locked",
      "test --workspace --locked",
      "build --workspace --release --locked",
      "package --locked",
    ]
  );

  ctx.clear_calls();
  assert_success(&ctx.run(&["test", "--no-run"], 0));
  assert_eq!(ctx.calls(), vec!["test --workspace --no-run"]);

  ctx.clear_calls();
  assert_success(&ctx.run(&["verify"], 0));
  assert_eq!(
    ctx.calls(),
    vec![
      "fmt --all -- --check",
      "check --workspace --all-targets --locked",
      "test --workspace --locked",
    ]
  );

  ctx.clear_calls();
  let version = ctx.run(&["version"], 0);
  assert_success(&version);
  assert!(String::from_utf8_lossy(&version.stdout).contains("1.0.0"));
  assert!(ctx.calls().is_empty());

  let completions = ctx.run(&["completions", "bash"], 0);
  assert_success(&completions);
  assert!(String::from_utf8_lossy(&completions.stdout).contains("xtask"));
  assert!(ctx.calls().is_empty());
}

#[test]
fn command_failure_is_reported_as_a_nonzero_exit() {
  let ctx = TestContext::new();
  let output = ctx.run(&["check"], 17);

  assert!(!output.status.success());
  assert_eq!(output.status.code(), Some(1));
  assert!(
    String::from_utf8_lossy(&output.stderr).contains("exited with"),
    "expected Cargo's failure to be reported: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  assert_eq!(ctx.calls(), vec!["check --workspace"]);
}

#[test]
fn release_rejects_a_version_that_does_not_match_the_workspace() {
  let ctx = TestContext::new();
  let output = ctx.run(&["release", "v9.9.9"], 0);

  assert!(!output.status.success());
  assert_eq!(output.status.code(), Some(1));
  assert!(
    String::from_utf8_lossy(&output.stderr).contains("does not match requested"),
    "unexpected release validation error: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  assert!(ctx.calls().is_empty());
}

#[test]
fn cargo_spawn_failure_is_reported() {
  let ctx = TestContext::new();
  fs::remove_file(ctx.bin.join("cargo")).expect("fake Cargo should be removed");

  let output = ctx.run_without_cargo_in_path(&["check"]);
  assert!(!output.status.success());
  assert_eq!(output.status.code(), Some(1));
  assert!(
    String::from_utf8_lossy(&output.stderr).contains("failed to start"),
    "unexpected spawn error: {}",
    String::from_utf8_lossy(&output.stderr)
  );
}
