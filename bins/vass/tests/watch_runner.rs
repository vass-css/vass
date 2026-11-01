//! Watch-mode scenario integration test runner for VASS.
//!
//! Port of the Python `tests/watch/run_watch_tests.py`. Drives the real
//! `vass watch` process against temp copies of input files and verifies the
//! documented watch semantics.
//!
//! Run with: `cargo test --test watch_runner`

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use regex::Regex;

fn bass_bin() -> PathBuf {
  std::env::var("BASS_BIN")
    .map(PathBuf::from)
    .unwrap_or_else(|_| PathBuf::from(env!("CARGO_BIN_EXE_vass")))
}

static TS_RE: OnceLock<Regex> = OnceLock::new();

fn ts_re() -> &'static Regex {
  TS_RE.get_or_init(|| Regex::new(r"\[\d\d:\d\d:\d\d\] ").unwrap())
}

struct Watch {
  log_path: PathBuf,
  log_file: Option<fs::File>,
  child: Option<std::process::Child>,
  final_log: String,
}

impl Watch {
  fn new(input_name: &str, workdir: &Path) -> std::io::Result<Self> {
    let log_path = workdir.join("watch.log");
    let log_file = fs::File::create(&log_path)?;
    let child = Command::new(bass_bin())
      .arg("watch")
      .arg(input_name)
      .arg("--poll-interval")
      .arg("50")
      .current_dir(workdir)
      .stdout(log_file.try_clone()?)
      .stderr(Stdio::from(log_file.try_clone()?))
      .spawn()?;
    Ok(Self {
      log_path,
      log_file: Some(log_file),
      child: Some(child),
      final_log: String::new(),
    })
  }

  fn log_text(&mut self) -> String {
    if let Some(f) = self.log_file.as_mut() {
      let _ = f.flush();
    }
    let Ok(text) = fs::read_to_string(&self.log_path) else {
      return self.final_log.clone();
    };
    ts_re().replace_all(&text, "[TS] ").to_string()
  }

  fn alive(&mut self) -> bool {
    self
      .child
      .as_mut()
      .is_some_and(|child| matches!(child.try_wait(), Ok(None) | Err(_)))
  }

  fn stop(&mut self) -> Option<i32> {
    if let Some(mut child) = self.child.take() {
      #[cfg(unix)]
      {
        unsafe {
          let _ = libc::kill(child.id() as libc::pid_t, libc::SIGINT);
        }
      }
      let rc = wait_for_exit(&mut child, Duration::from_secs(8));
      let rc = rc.or_else(|| {
        child.kill().ok();
        child.wait().ok().and_then(|s| s.code())
      });
      if let Some(f) = self.log_file.take() {
        drop(f);
      }
      self.final_log = fs::read_to_string(&self.log_path)
        .unwrap_or_default()
        .lines()
        .map(|l| ts_re().replace_all(l, "[TS] ").to_string())
        .collect::<Vec<_>>()
        .join("\n");
      rc
    } else {
      None
    }
  }
}

/// Wait for a child process to exit, polling `try_wait`, up to `timeout`.
/// Returns `Some(exit_code)` if it exits, `None` on timeout.
fn wait_for_exit(child: &mut std::process::Child, timeout: Duration) -> Option<i32> {
  let deadline = Instant::now() + timeout;
  while Instant::now() < deadline {
    match child.try_wait() {
      Ok(Some(status)) => return status.code(),
      Ok(None) => std::thread::sleep(Duration::from_millis(100)),
      Err(_) => return None,
    }
  }
  None
}

fn write(path: &Path, text: &str) {
  if let Some(p) = path.parent() {
    let _ = fs::create_dir_all(p);
  }
  let mut f = fs::File::create(path).expect("failed to write test file");
  let _ = f.write_all(text.as_bytes());
}

fn read(path: &Path) -> Option<String> {
  fs::read_to_string(path).ok()
}

fn wait_for<F>(mut pred: F, timeout: Duration) -> bool
where
  F: FnMut() -> bool,
{
  let deadline = Instant::now() + timeout;
  while Instant::now() < deadline {
    if pred() {
      return true;
    }
    std::thread::sleep(Duration::from_millis(100));
  }
  false
}

// ------------------------------------------------------------------ scenarios

fn sc_file_change(tmp: &Path) -> Result<(), String> {
  write(&tmp.join("in.vcss"), "a { b: 1; }\n");
  let mut w = Watch::new("in.vcss", tmp).map_err(|e| e.to_string())?;
  let result = (|| {
    if !wait_for(
      || w.log_text().contains("initial compile OK"),
      Duration::from_secs(8),
    ) {
      return Err(format!("no initial compile; log={:?}", w.log_text()));
    }
    let css = tmp.join("in.css");
    if !wait_for(
      || read(&css).as_deref() == Some("a {\n  b: 1;\n}\n"),
      Duration::from_secs(8),
    ) {
      return Err(format!("initial output wrong: {:?}", read(&css)));
    }
    write(&tmp.join("in.vcss"), "a { b: 2; }\n");
    if !wait_for(
      || w.log_text().contains("recompiled OK"),
      Duration::from_secs(8),
    ) {
      return Err(format!("no recompile; log={:?}", w.log_text()));
    }
    if !wait_for(
      || read(&css).as_deref() == Some("a {\n  b: 2;\n}\n"),
      Duration::from_secs(8),
    ) {
      return Err(format!("output not updated: {:?}", read(&css)));
    }
    Ok(())
  })();
  let _ = w.stop();
  result
}

fn sc_rapid_changes(tmp: &Path) -> Result<(), String> {
  write(&tmp.join("in.vcss"), "a { b: 0; }\n");
  let mut w = Watch::new("in.vcss", tmp).map_err(|e| e.to_string())?;
  let result = (|| {
    let _ = wait_for(
      || w.log_text().contains("initial compile OK"),
      Duration::from_secs(8),
    );
    let css = tmp.join("in.css");
    let _ = wait_for(|| read(&css).is_some(), Duration::from_secs(8));
    for i in 1..=5 {
      write(&tmp.join("in.vcss"), &format!("a {{ b: {}; }}\n", i));
      std::thread::sleep(Duration::from_millis(120));
    }
    if !wait_for(
      || read(&css).as_deref() == Some("a {\n  b: 5;\n}\n"),
      Duration::from_secs(10),
    ) {
      return Err(format!("final output not last content: {:?}", read(&css)));
    }
    if !w.alive() {
      return Err("watch died during rapid changes".to_string());
    }
    Ok(())
  })();
  let _ = w.stop();
  result
}

fn sc_file_creation(tmp: &Path) -> Result<(), String> {
  write(&tmp.join("in.vcss"), "a { b: 7; }\n");
  let mut w = Watch::new("in.vcss", tmp).map_err(|e| e.to_string())?;
  let result = (|| {
    if !wait_for(
      || w.log_text().contains("initial compile OK"),
      Duration::from_secs(8),
    ) {
      return Err(format!("no initial compile; log={:?}", w.log_text()));
    }
    if !wait_for(
      || read(&tmp.join("in.css")).is_some(),
      Duration::from_secs(8),
    ) {
      return Err("output file was not created".to_string());
    }
    Ok(())
  })();
  let _ = w.stop();
  result
}

fn sc_file_deletion(tmp: &Path) -> Result<(), String> {
  write(&tmp.join("in.vcss"), "a { b: 1; }\n");
  let mut w = Watch::new("in.vcss", tmp).map_err(|e| e.to_string())?;
  let result = (|| {
    let _ = wait_for(
      || w.log_text().contains("initial compile OK"),
      Duration::from_secs(8),
    );
    let css = tmp.join("in.css");
    let _ = wait_for(|| read(&css).is_some(), Duration::from_secs(8));
    fs::remove_file(tmp.join("in.vcss")).ok();
    std::thread::sleep(Duration::from_millis(1200));
    if !w.alive() {
      return Err("watch process died after input deletion".to_string());
    }
    if read(&css).as_deref() != Some("a {\n  b: 1;\n}\n") {
      return Err(format!("last good output not preserved: {:?}", read(&css)));
    }
    write(&tmp.join("in.vcss"), "a { b: 3; }\n");
    if !wait_for(
      || read(&css).as_deref() == Some("a {\n  b: 3;\n}\n"),
      Duration::from_secs(10),
    ) {
      return Err(format!("no recompile after recreation: {:?}", read(&css)));
    }
    Ok(())
  })();
  let _ = w.stop();
  result
}

fn sc_file_rename(tmp: &Path) -> Result<(), String> {
  write(&tmp.join("in.vcss"), "a { b: 1; }\n");
  let mut w = Watch::new("in.vcss", tmp).map_err(|e| e.to_string())?;
  let result = (|| {
    let _ = wait_for(
      || w.log_text().contains("initial compile OK"),
      Duration::from_secs(8),
    );
    let css = tmp.join("in.css");
    let _ = wait_for(|| read(&css).is_some(), Duration::from_secs(8));
    fs::rename(tmp.join("in.vcss"), tmp.join("moved.vcss")).ok();
    std::thread::sleep(Duration::from_millis(1200));
    if !w.alive() {
      return Err("watch process died after input rename".to_string());
    }
    if read(&css).as_deref() != Some("a {\n  b: 1;\n}\n") {
      return Err("output changed after rename".to_string());
    }
    Ok(())
  })();
  let _ = w.stop();
  result
}

fn sc_ignored_paths(tmp: &Path) -> Result<(), String> {
  write(&tmp.join("in.vcss"), "a { b: 1; }\n");
  write(&tmp.join("other.vcss"), "z { y: 9; }\n");
  let mut w = Watch::new("in.vcss", tmp).map_err(|e| e.to_string())?;
  let result = (|| {
    let _ = wait_for(
      || w.log_text().contains("initial compile OK"),
      Duration::from_secs(8),
    );
    let css = tmp.join("in.css");
    let _ = wait_for(|| read(&css).is_some(), Duration::from_secs(8));
    let before = w.log_text();
    let mtime = fs::metadata(&css).ok().and_then(|m| m.modified().ok());
    for _ in 0..3 {
      write(
        &tmp.join("other.vcss"),
        &format!(
          "z {{ y: {}; }}\n",
          std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
            % 97
        ),
      );
      std::thread::sleep(Duration::from_millis(150));
    }
    std::thread::sleep(Duration::from_millis(1200));
    let after = w.log_text();
    if after != before {
      return Err(format!(
        "unrelated file change triggered activity: {:?}",
        after
      ));
    }
    let mtime2 = fs::metadata(&css).ok().and_then(|m| m.modified().ok());
    if mtime != mtime2 {
      return Err("output rewritten by unrelated change".to_string());
    }
    if !w.alive() {
      return Err("watch died".to_string());
    }
    Ok(())
  })();
  let _ = w.stop();
  result
}

fn sc_compilation_error(tmp: &Path) -> Result<(), String> {
  write(&tmp.join("in.vcss"), "a { b: 1; }\n");
  let mut w = Watch::new("in.vcss", tmp).map_err(|e| e.to_string())?;
  let result = (|| {
    let _ = wait_for(
      || w.log_text().contains("initial compile OK"),
      Duration::from_secs(8),
    );
    let css = tmp.join("in.css");
    let _ = wait_for(|| read(&css).is_some(), Duration::from_secs(8));
    write(&tmp.join("in.vcss"), "a { b: 1px + 1s; }\n");
    if !wait_for(|| w.log_text().contains("E0003"), Duration::from_secs(8)) {
      return Err(format!(
        "compile error not reported; log={:?}",
        w.log_text()
      ));
    }
    if !w.alive() {
      return Err("watch died after compile error".to_string());
    }
    if read(&css).as_deref() != Some("a {\n  b: 1;\n}\n") {
      return Err(format!(
        "previous output clobbered by failed compile: {:?}",
        read(&css)
      ));
    }
    Ok(())
  })();
  let _ = w.stop();
  result
}

fn sc_recovery_after_error(tmp: &Path) -> Result<(), String> {
  write(&tmp.join("in.vcss"), "a { b: 1px + 1s; }\n");
  let mut w = Watch::new("in.vcss", tmp).map_err(|e| e.to_string())?;
  let result = (|| {
    if !wait_for(|| w.log_text().contains("E0003"), Duration::from_secs(8)) {
      return Err(format!(
        "initial error not reported; log={:?}",
        w.log_text()
      ));
    }
    write(&tmp.join("in.vcss"), "a { b: 4px + 5px; }\n");
    let css = tmp.join("in.css");
    if !wait_for(
      || read(&css).as_deref() == Some("a {\n  b: 9px;\n}\n"),
      Duration::from_secs(10),
    ) {
      return Err(format!(
        "no recovery recompile; log={:?} css={:?}",
        w.log_text(),
        read(&css)
      ));
    }
    if !w.alive() {
      return Err("watch died during recovery".to_string());
    }
    Ok(())
  })();
  let _ = w.stop();
  result
}

fn sc_graceful_shutdown(tmp: &Path) -> Result<(), String> {
  write(&tmp.join("in.vcss"), "a { b: 1; }\n");
  let mut w = Watch::new("in.vcss", tmp).map_err(|e| e.to_string())?;
  (|| {
    let _ = wait_for(
      || w.log_text().contains("initial compile OK"),
      Duration::from_secs(8),
    );
    let rc = w.stop();
    let log = w.log_text();
    if !log.contains("Stopping watch mode...") {
      return Err(format!("no shutdown message; log={log:?}"));
    }
    if rc.is_none() {
      return Err("watch did not exit on SIGINT".to_string());
    }
    Ok(())
  })()
}

type Scenario = (&'static str, fn(&Path) -> Result<(), String>);

const SCENARIOS: &[Scenario] = &[
  ("file-change", sc_file_change),
  ("rapid-changes", sc_rapid_changes),
  ("file-creation", sc_file_creation),
  ("file-deletion", sc_file_deletion),
  ("file-rename", sc_file_rename),
  ("ignored-paths", sc_ignored_paths),
  ("compilation-error", sc_compilation_error),
  ("recovery-after-error", sc_recovery_after_error),
  ("graceful-shutdown", sc_graceful_shutdown),
];

/// Runs all watch scenarios and asserts none fail.
#[test]
fn run_all_watch_scenarios() {
  let mut fails = 0i32;
  let mut pass_count = 0i32;
  let mut skip_count = 0i32;

  for (name, fn_) in SCENARIOS {
    if !bass_bin().exists() {
      println!(
        "RESULT SKIP {name} NOT-VERIFIED vass binary not found at {:?}",
        bass_bin()
      );
      skip_count += 1;
      continue;
    }
    let tmp = tempfile::Builder::new()
      .prefix("vass-watch-")
      .tempdir()
      .expect("failed to create temp dir");
    match fn_(tmp.path()) {
      Ok(()) => {
        println!("RESULT PASS {name}");
        pass_count += 1;
      }
      Err(detail) => {
        println!("RESULT FAIL {name} {detail}");
        fails += 1;
      }
    }
  }

  println!(
    "\nwatch scenarios: {}, passed: {}, failed: {}, skipped: {}",
    SCENARIOS.len(),
    pass_count,
    fails,
    skip_count
  );

  assert_eq!(fails, 0, "{fails} watch scenario(s) failed");
}
