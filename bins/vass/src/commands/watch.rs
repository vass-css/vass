//! Watch command
//!
//! Recompiles the input file whenever it changes. Uses a dependency-free
//! polling watcher (mtime + size) so it works identically on every platform
//! and keeps cross-compilation trivial.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use clap::Args as ClapArgs;
use vass_transpiler::OutputStyle;

/// Debounce window: file must be stable for this long before recompiling.
const DEBOUNCE: Duration = Duration::from_millis(60);

/// Watch files for changes and recompile
#[derive(ClapArgs, Debug)]
pub struct Args {
  /// Input VASS file to watch
  #[arg(value_name = "INPUT")]
  pub input: String,

  /// Output CSS file (default: input path with a .css extension)
  #[arg(short, long, value_name = "OUTPUT")]
  pub output: Option<String>,

  /// Polling interval in milliseconds
  #[arg(long, default_value = "150")]
  pub poll_interval: u64,
}

/// State of the watched file between polls.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct FileStamp {
  mtime: SystemTime,
  len: u64,
}

impl FileStamp {
  fn of(path: &Path) -> Option<FileStamp> {
    let meta = std::fs::metadata(path).ok()?;
    Some(FileStamp {
      mtime: meta.modified().ok()?,
      len: meta.len(),
    })
  }
}

/// Derive the default output path: `style.vass` -> `style.css`,
/// preserving parent directories and case-insensitive extension handling.
fn derive_css_path(input: &Path) -> PathBuf {
  let mut out = input.to_path_buf();
  out.set_extension("css");
  out
}

/// Run the watch command
pub async fn run(args: Args) -> anyhow::Result<()> {
  let input = PathBuf::from(&args.input);
  let output: PathBuf = match &args.output {
    Some(p) => PathBuf::from(p),
    None => derive_css_path(&input),
  };
  let tick = Duration::from_millis(args.poll_interval.max(50));

  if !input.exists() {
    anyhow::bail!("File not found: {}", input.display());
  }

  println!(
    "Watching {} -> {}  (Ctrl+C to stop)",
    input.display(),
    output.display()
  );

  // Initial compile. A broken input must NOT kill the watcher: the user
  // may have invoked `watch` while the file is mid-edit, and the whole
  // point of watch mode is to recover on the next save (same semantics as
  // the mid-watch error path below and as dart-sass `--watch`). Only a
  // MISSING input is fatal (checked above).
  let style = OutputStyle::Expanded;
  let started = std::time::Instant::now();
  match super::compile::compile_file(&input, Some(&output), style) {
    Ok(bytes) => println!(
      "[{}] initial compile OK: {} ({} bytes, {} ms)",
      timestamp(),
      output.display(),
      bytes,
      started.elapsed().as_millis()
    ),
    Err(e) => {
      // Keep watching; `e` already carries the rustc-style diagnostic.
      eprintln!("[{}]", timestamp());
      eprintln!("{e}");
      eprintln!();
      eprintln!("Build failed - vass keeps watching. Fix the error and save to retry.");
    }
  }

  let mut last = FileStamp::of(&input).expect("input exists, checked above");

  loop {
    // Wait for either the next poll tick or Ctrl+C.
    tokio::select! {
        _ = tokio::time::sleep(tick) => {}
        _ = tokio::signal::ctrl_c() => {
            println!("\nStopping watch mode...");
            return Ok(());
        }
    }

    let Some(now) = FileStamp::of(&input) else {
      // File temporarily missing (editor save-in-place): keep waiting.
      continue;
    };

    if now == last {
      continue;
    }

    // A zero-length file is almost always the transient window of an
    // editor save-in-place (truncate, then write). Compiling it would
    // clobber the last good CSS with an empty file — skip and wait.
    if now.len == 0 {
      last = now;
      eprintln!(
        "[{}] input is empty (mid-save?) — keeping last output",
        timestamp()
      );
      continue;
    }

    // Debounce: wait for the file to settle before compiling.
    tokio::time::sleep(DEBOUNCE).await;
    let settled = FileStamp::of(&input).unwrap_or(now);
    if settled != now {
      // Still changing; the next tick will pick it up again.
      continue;
    }

    last = settled;
    let started = std::time::Instant::now();
    match super::compile::compile_file(&input, Some(&output), style) {
      Ok(bytes) => println!(
        "[{}] recompiled OK: {} ({} bytes, {} ms)",
        timestamp(),
        output.display(),
        bytes,
        started.elapsed().as_millis()
      ),
      Err(e) => {
        // Keep watching even when the user is mid-edit. The output
        // file is only written after a successful compile, so the
        // last good CSS always survives a broken intermediate state.
        // `e` already carries the rustc-style diagnostic block.
        eprintln!("[{}]", timestamp());
        eprintln!("{e}");
        eprintln!();
        eprintln!("Build failed - vass keeps watching. Fix the error and save to retry.");
      }
    }
  }
}

/// Human-readable local timestamp for watch log lines.
fn timestamp() -> String {
  let secs = SystemTime::now()
    .duration_since(SystemTime::UNIX_EPOCH)
    .map(|d| d.as_secs())
    .unwrap_or(0);
  format!(
    "{:02}:{:02}:{:02}",
    (secs / 3600) % 24,
    (secs / 60) % 60,
    secs % 60
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_derive_css_path_basic() {
    assert_eq!(
      derive_css_path(Path::new("style.vass")),
      PathBuf::from("style.css")
    );
  }

  #[test]
  fn test_derive_css_path_dirs_and_vcss() {
    assert_eq!(
      derive_css_path(Path::new("examples/test.vcss")),
      PathBuf::from("examples/test.css")
    );
  }

  #[test]
  fn test_derive_css_path_no_extension() {
    assert_eq!(
      derive_css_path(Path::new("style")),
      PathBuf::from("style.css")
    );
  }

  #[test]
  fn test_file_stamp_detects_change() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("w.vass");
    std::fs::write(&f, "a\n  color: red\n").unwrap();
    let s1 = FileStamp::of(&f).unwrap();
    // Different content -> different length -> stamp changes even with
    // coarse mtime granularity on some filesystems.
    std::thread::sleep(std::time::Duration::from_millis(10));
    std::fs::write(&f, "a\n  color: blue;\n\n\n").unwrap();
    let s2 = FileStamp::of(&f).unwrap();
    assert_ne!(s1, s2);
  }
}
