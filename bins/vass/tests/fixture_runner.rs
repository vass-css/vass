//! Fixture-based integration test runner for VASS.
//!
//! Port of the Python `tests/run_tests.py`. Runs `.vcss`, `.cli`, `.int` and
//! `.watch` fixtures under `tests/` against the `vass` binary.
//!
//! Run with: `cargo test --test fixture_runner`

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use regex::Regex;

/// Root of the `tests/` directory (workspace-root `tests/`, i.e.
/// `bins/vass/../tests` relative to this crate's manifest dir).
const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests");

/// Resolve the VASS binary under test.
///
/// Explicit overrides take precedence. Cargo supplies CARGO_BIN_EXE_vass
/// when compiling integration tests, so this remains correct when a tool
/// such as cargo-llvm-cov uses a separate target directory.
fn resolve_bass_bin(override_path: Option<std::ffi::OsString>, cargo_bin: Option<&str>) -> PathBuf {
  override_path
    .map(PathBuf::from)
    .or_else(|| cargo_bin.map(PathBuf::from))
    .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/vass"))
}

fn bass_bin() -> PathBuf {
  resolve_bass_bin(
    std::env::var_os("BASS_BIN"),
    option_env!("CARGO_BIN_EXE_vass"),
  )
}


#[cfg(test)]
mod binary_resolution_tests {
  use super::*;

  #[test]
  fn test_bass_bin_prefers_explicit_override() {
    let path = resolve_bass_bin(
      Some(std::ffi::OsString::from("custom/vass")),
      Some("cargo/vass"),
    );
    assert_eq!(path, PathBuf::from("custom/vass"));
  }

  #[test]
  fn test_bass_bin_uses_cargo_binary_path() {
    let path = resolve_bass_bin(None, Some("cargo-target/debug/vass"));
    assert_eq!(path, PathBuf::from("cargo-target/debug/vass"));
  }

  #[test]
  fn test_bass_bin_uses_default_path_without_overrides() {
    let path = resolve_bass_bin(None, None);
    let expected = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/vass");
    assert_eq!(path, expected);
  }
}

static DEBUG_RE: OnceLock<Regex> = OnceLock::new();

fn debug_re() -> &'static Regex {
  DEBUG_RE.get_or_init(|| Regex::new(r"(?:DEBUG|WARNING): (.*)$").unwrap())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
  Pass,
  Fail,
  XFail,
  XPass,
  Skip,
  Invalid,
  Unsupported,
  NotApplicable,
  NotVerified,
}

impl Status {
  fn as_str(self) -> &'static str {
    match self {
      Status::Pass => "PASS",
      Status::Fail => "FAIL",
      Status::XFail => "XFAIL",
      Status::XPass => "XPASS",
      Status::Skip => "SKIP",
      Status::Invalid => "INVALID",
      Status::Unsupported => "UNSUPPORTED",
      Status::NotApplicable => "NOT-APPLICABLE",
      Status::NotVerified => "NOT-VERIFIED",
    }
  }
}

const KINDS: &[&str] = &[".vcss", ".cli", ".int", ".watch"];

fn discover(prefix: &str) -> Vec<String> {
  let root = Path::new(ROOT)
    .canonicalize()
    .unwrap_or_else(|_| Path::new(ROOT).to_path_buf());
  let mut out = Vec::new();
  collect_files(&root, &mut out, &root);
  out.retain(|rel| {
    let name = Path::new(rel)
      .file_name()
      .and_then(|n| n.to_str())
      .unwrap_or("");
    if !KINDS.iter().any(|k| name.ends_with(k)) {
      return false;
    }
    if name.ends_with(".expected.vcss") {
      return false;
    }
    if rel.split('/').any(|c| c == "fixtures") {
      return false;
    }
    if !prefix.is_empty() && !rel.starts_with(prefix) {
      return false;
    }
    true
  });
  out.sort();
  out
}

fn collect_files(dir: &Path, out: &mut Vec<String>, root: &Path) {
  let Ok(entries) = fs::read_dir(dir) else {
    return;
  };
  for entry in entries.flatten() {
    let path = entry.path();
    if path.is_dir() {
      collect_files(&path, out, root);
    } else if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
      if name.starts_with('_') {
        continue;
      }
      if let Ok(rel) = path.strip_prefix(root) {
        out.push(rel.to_string_lossy().replace('\\', "/"));
      }
    }
  }
}

fn read(path: &Path) -> Option<String> {
  fs::read_to_string(path).ok()
}

fn dbg_values(stderr: &str) -> Vec<String> {
  let re = debug_re();
  let mut vals = Vec::new();
  for line in stderr.lines() {
    if let Some(caps) = re.captures(line) {
      vals.push(caps[1].to_string());
    }
  }
  vals
}

fn dbg_matches(expected_lines: &[&str], stderr: &str) -> bool {
  let vals = dbg_values(stderr);
  let mut i = 0usize;
  for exp in expected_lines {
    let exp = exp.trim();
    while i < vals.len() && !vals[i].contains(exp) {
      i += 1;
    }
    if i >= vals.len() {
      return false;
    }
    i += 1;
  }
  true
}

fn css_equal(expected: &str, actual: &str) -> bool {
  if expected == actual {
    return true;
  }
  expected.trim_end_matches('\n') == actual.trim_end_matches('\n')
    && expected.ends_with('\n') == actual.ends_with('\n')
}

fn apply_matcher(spec: &str, text: &str) -> Option<String> {
  let trunc = |s: &str| {
    let len = 120.min(s.len());
    s[..len].to_string()
  };
  if spec == "empty" {
    return if text.is_empty() {
      None
    } else {
      Some(format!("expected empty output, got {:?}", trunc(text)))
    };
  }
  if let Some(want) = spec.strip_prefix("exact:") {
    return if text == want {
      None
    } else {
      Some(format!(
        "exact mismatch: want {:?} got {:?}",
        trunc(want),
        trunc(text)
      ))
    };
  }
  if let Some(sub) = spec.strip_prefix("has:") {
    return if text.contains(sub) {
      None
    } else {
      Some(format!(
        "missing substring {:?} in {:?}",
        trunc(sub),
        trunc(text)
      ))
    };
  }
  if let Some(pat) = spec.strip_prefix("re:") {
    match Regex::new(&format!("(?m){pat}")) {
      Ok(re) => {
        return if re.is_match(text) {
          None
        } else {
          Some(format!(
            "regex {:?} not found in {:?}",
            trunc(pat),
            trunc(text)
          ))
        };
      }
      Err(_) => return Some(format!("invalid regex {:?}", trunc(pat))),
    }
  }
  if text.contains(spec) {
    None
  } else {
    Some(format!(
      "missing substring {:?} in {:?}",
      trunc(spec),
      trunc(text)
    ))
  }
}

fn substitute(line: &str, env: &HashMap<String, String>) -> String {
  let mut out = line.to_string();
  for (k, v) in env {
    out = out.replace(&format!("{{{k}}}"), v);
  }
  out
}

fn shlex_split(s: &str) -> Vec<String> {
  let mut out = Vec::new();
  let mut cur = String::new();
  let mut in_single = false;
  let mut in_double = false;
  let mut escaped = false;
  for c in s.chars() {
    if escaped {
      cur.push(c);
      escaped = false;
      continue;
    }
    match c {
      '\'' if !in_double => in_single = !in_single,
      '"' if !in_single => in_double = !in_double,
      '\\' => escaped = true,
      c if c.is_whitespace() && !in_single && !in_double => {
        if !cur.is_empty() {
          out.push(std::mem::take(&mut cur));
        }
      }
      c => cur.push(c),
    }
  }
  if !cur.is_empty() {
    out.push(cur);
  }
  out
}

#[derive(Debug, Clone, Default)]
struct RunBlock {
  args: String,
  bass_args: bool,
  stdin: String,
  exit: i32,
  stdout: Vec<String>,
  stderr: Vec<String>,
  file_has: Vec<(String, String)>,
  nofile: Vec<String>,
}

type ParsedSpec = (Vec<(String, String)>, Vec<String>, Vec<RunBlock>);

fn parse_spec(path: &Path, env: &HashMap<String, String>) -> Result<ParsedSpec, String> {
  let text = read(path).unwrap_or_default();
  let mut setup: Vec<(String, String)> = Vec::new();
  let mut pres: Vec<String> = Vec::new();
  let mut blocks: Vec<RunBlock> = Vec::new();
  let mut cur_idx: Option<usize> = None;
  let mut in_heredoc: Option<(String, Vec<String>)> = None;
  let mut pending_file: Option<String> = None;

  for raw in text.lines() {
    if let Some((rel, lines)) = in_heredoc.as_mut() {
      if raw.trim() == ">>>" {
        setup.push((rel.clone(), lines.join("\n") + "\n"));
        in_heredoc = None;
      } else {
        lines.push(raw.to_string());
      }
      continue;
    }
    let line = raw.trim();
    if line.is_empty() || line.starts_with('#') {
      continue;
    }
    if let Some(raw_rel) = line.strip_prefix("<<<") {
      let raw_rel = raw_rel.trim();
      let rel = if raw_rel.is_empty() {
        pending_file
          .take()
          .ok_or_else(|| format!("{path:?}: '<<<' without a file name"))?
      } else {
        // Preserve spaces in filenames (quoted or bare multi-token).
        let tokens = shlex_split(raw_rel);
        if tokens.is_empty() {
          substitute(raw_rel, env)
        } else if tokens.len() == 1 {
          substitute(&tokens[0], env)
        } else {
          substitute(&tokens.join(" "), env)
        }
      };
      pending_file = None;
      in_heredoc = Some((rel, Vec::new()));
      continue;
    }
    let (key, val) = line.split_once(':').unwrap_or((line, ""));
    let key = key.trim();
    let val = val.trim();
    match key {
      "pre" => pres.push(substitute(val, env)),
      "file" => {
        if val.contains(" | ") {
          let mut parts = val.splitn(2, " | ");
          let name = parts.next().unwrap_or("");
          let content = parts.next().unwrap_or("");
          let name = shlex_split(name)
            .into_iter()
            .next()
            .map(|s| substitute(&s, env))
            .unwrap_or_else(|| name.to_string());
          let content = substitute(content, env).replace("\\n", "\n");
          setup.push((name, content));
        } else {
          // Filenames may contain spaces. shlex_split handles quoted
          // values (`"my dir/in.vcss"` -> `my dir/in.vcss`); for
          // unquoted multi-token values we join the tokens back so the
          // whole path survives.
          let tokens = shlex_split(val);
          let name = if tokens.is_empty() {
            substitute(val, env)
          } else if tokens.len() == 1 {
            substitute(&tokens[0], env)
          } else {
            substitute(&tokens.join(" "), env)
          };
          pending_file = Some(name);
        }
      }
      "run" | "args" => {
        let args = substitute(val, env).replace("\\n", "\n");
        blocks.push(RunBlock {
          args,
          bass_args: key == "args",
          ..Default::default()
        });
        cur_idx = Some(blocks.len() - 1);
      }
      "stdout" | "stderr" | "stdin" => {
        let v = substitute(val, env).replace("\\n", "\n");
        let Some(idx) = cur_idx else { continue };
        match key {
          "stdin" => blocks[idx].stdin = v,
          "stdout" => blocks[idx].stdout.push(v),
          "stderr" => blocks[idx].stderr.push(v),
          _ => unreachable!(),
        }
      }
      "exit" => {
        let Some(idx) = cur_idx else { continue };
        blocks[idx].exit = val
          .parse::<i32>()
          .map_err(|_| format!("{path:?}: invalid exit code {val:?}"))?;
      }
      "file-has" => {
        let Some(idx) = cur_idx else { continue };
        let spec = substitute(val, env);
        let (rel, rest) = spec.split_once(':').unwrap_or((&spec, ""));
        blocks[idx]
          .file_has
          .push((rel.to_string(), rest.to_string()));
      }
      "nofile" => {
        let Some(idx) = cur_idx else { continue };
        blocks[idx].nofile.push(substitute(val, env));
      }
      _ => return Err(format!("{path:?}: unknown key {key:?}")),
    }
  }
  if in_heredoc.is_some() {
    return Err(format!("{path:?}: unterminated heredoc"));
  }
  if let Some(pf) = pending_file {
    return Err(format!("{path:?}: 'file: {pf}' has no '<<<' body"));
  }
  Ok((setup, pres, blocks))
}

fn run_blocks_fixture(rel: &str, _kind: &str) -> (Status, Option<String>) {
  let base = Path::new(ROOT).join(rel);
  let dir = base.parent().unwrap_or(Path::new("")).to_path_buf();
  let mut env: HashMap<String, String> = HashMap::new();
  env.insert("vass".to_string(), bass_bin().to_string_lossy().to_string());
  env.insert("tests".to_string(), ROOT.to_string());
  env.insert("dir".to_string(), dir.to_string_lossy().to_string());
  env.insert("tmp".to_string(), String::new());

  let tmp = tempfile::Builder::new()
    .prefix("vass-test-")
    .tempdir()
    .expect("failed to create temp dir");
  env.insert("tmp".to_string(), tmp.path().to_string_lossy().to_string());

  let (setup, pres, blocks) = match parse_spec(&base, &env) {
    Ok(v) => v,
    Err(e) => return (Status::Invalid, Some(e)),
  };

  if blocks.is_empty() {
    return (Status::Invalid, Some("no run: block".to_string()));
  }

  for (rpath, content) in &setup {
    let dest = tmp.path().join(rpath);
    if let Some(p) = dest.parent() {
      let _ = fs::create_dir_all(p);
    }
    if let Ok(mut f) = fs::File::create(&dest) {
      let _ = f.write_all(content.as_bytes());
    }
  }

  for cmd in &pres {
    let p = Command::new("bash")
      .arg("-c")
      .arg(cmd)
      .current_dir(tmp.path())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .output();
    match p {
      Ok(out) if out.status.success() => {}
      Ok(out) => {
        let err = String::from_utf8_lossy(&out.stderr);
        return (
          Status::Fail,
          Some(format!(
            "pre command failed ({cmd}): {}",
            err.lines().last().unwrap_or("")
          )),
        );
      }
      Err(e) => {
        return (
          Status::Fail,
          Some(format!("pre command failed ({cmd}): {e}")),
        );
      }
    }
  }

  for blk in &blocks {
    // Preserve shell quoting semantics exactly: run the raw args string
    // through `bash -c` so quoted paths with spaces survive intact.
    // `{vass}` is already substituted by `substitute()`; a bare leading
    // `vass` token is rewritten to the real binary path here.
    let raw = substitute(&blk.args, &env);
    let raw = if blk.bass_args {
      raw
    } else if raw == "vass" {
      bass_bin().to_string_lossy().to_string()
    } else if let Some(rest) = raw.strip_prefix("vass ") {
      format!("{} {}", bass_bin().to_string_lossy(), rest)
    } else {
      // Many fixtures invoke Vass subcommands directly (e.g. `exec ...`,
      // `fmt ...`) without a leading `vass` token. Rewrite those to use
      // the real binary path so they do not resolve to nonexistent
      // executables on PATH.
      let cmd = raw.split(char::is_whitespace).next().unwrap_or("");
      let subcommands = [
        "compile", "run", "build", "check", "exec", "fmt", "lint", "repl", "watch", "lsp", "init",
        "doctor", "explain", "list", "stats", "version", "help",
      ];
      if subcommands.contains(&cmd) {
        format!("{} {}", bass_bin().to_string_lossy(), raw)
      } else {
        raw
      }
    };

    // An empty `run:` line means "invoke vass with no arguments" — the
    // help/exit-code fixtures rely on that. Spawn the binary directly
    // so `bash -c ""` (which exits 0) doesn't mask the real exit code.
    let mut cmd = if blk.bass_args {
      let mut c = Command::new(bass_bin());
      c.args(shlex_split(&raw))
        .current_dir(tmp.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
      c
    } else if raw.trim().is_empty() {
      let mut c = Command::new(bass_bin());
      c.current_dir(tmp.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
      c
    } else {
      let mut c = Command::new("bash");
      c.arg("-c")
        .arg(&raw)
        .current_dir(tmp.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
      c
    };

    let mut child = match cmd.spawn() {
      Ok(c) => c,
      Err(e) => return (Status::Fail, Some(format!("failed to spawn command: {e}"))),
    };

    if !blk.stdin.is_empty()
      && let Some(mut stdin) = child.stdin.take()
    {
      let _ = stdin.write_all(blk.stdin.as_bytes());
    }

    let out = match child.wait_with_output() {
      Ok(o) => o,
      Err(e) => return (Status::Fail, Some(format!("command failed: {e}"))),
    };

    let code = out.status.code().unwrap_or(-1);
    if code != blk.exit {
      let err = String::from_utf8_lossy(&out.stderr);
      let out_s = String::from_utf8_lossy(&out.stdout);
      let last = err.lines().chain(out_s.lines()).last().unwrap_or("?");
      return (
        Status::Fail,
        Some(format!("exit={code} want {}: {}", blk.exit, last)),
      );
    }

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    for spec in &blk.stdout {
      if let Some(d) = apply_matcher(spec, &stdout) {
        return (Status::Fail, Some(format!("stdout {d}")));
      }
    }
    for spec in &blk.stderr {
      if let Some(d) = apply_matcher(spec, &stderr) {
        return (Status::Fail, Some(format!("stderr {d}")));
      }
    }
    for (rpath, want) in &blk.file_has {
      let got = read(&tmp.path().join(rpath));
      match got {
        None => return (Status::Fail, Some(format!("file {rpath:?} not created"))),
        Some(g) if !g.contains(want) => {
          return (
            Status::Fail,
            Some(format!(
              "file {rpath:?} missing {:?}",
              &want[..80.min(want.len())]
            )),
          );
        }
        _ => {}
      }
    }
    for rpath in &blk.nofile {
      if tmp.path().join(rpath).exists() {
        return (
          Status::Fail,
          Some(format!("file {rpath:?} should not exist")),
        );
      }
    }
  }

  (Status::Pass, None)
}

fn run_vcss_fixture(rel: &str) -> (Status, Option<String>) {
  let base = Path::new(ROOT).join(rel);
  let stem = base.with_extension("");

  let skip_path_str = format!("{}.skip", stem.display());
  let skip = read(Path::new(&skip_path_str));
  if let Some(skip) = skip {
    let kind = skip.split(':').next().unwrap_or("SKIP").trim();
    let status = match kind {
      "SKIP" => Status::Skip,
      "UNSUPPORTED" => Status::Unsupported,
      "NOT-APPLICABLE" => Status::NotApplicable,
      "NOT-VERIFIED" => Status::NotVerified,
      _ => Status::Skip,
    };
    return (status, Some(skip.trim().to_string()));
  }

  let xfail_path = format!("{}.xfail", stem.display());
  let xfail = Path::new(&xfail_path).exists();
  let exp_css_path = format!("{}.expected.css", stem.display());
  let exp_css = read(Path::new(&exp_css_path));
  let exp_err_path = format!("{}.expected.err", stem.display());
  let exp_err = read(Path::new(&exp_err_path));
  let exp_dbg_path = format!("{}.expected.dbg", stem.display());
  let exp_dbg = read(Path::new(&exp_dbg_path));
  let opt_path = format!("{}.opt", stem.display());
  let opt = read(Path::new(&opt_path))
    .map(|s| {
      s.split_whitespace()
        .map(|w| w.to_string())
        .collect::<Vec<_>>()
    })
    .unwrap_or_default();

  if exp_err.is_none() && exp_css.is_none() && !xfail {
    return (Status::Invalid, Some("no expectation file".to_string()));
  }

  let mut cmd = Command::new(bass_bin());
  cmd
    .arg("compile")
    .arg(&base)
    .args(&opt)
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());

  let p = match cmd.output() {
    Ok(o) => o,
    Err(e) => return (Status::Fail, Some(format!("failed to run vass: {e}"))),
  };

  if let Some(exp_err) = &exp_err {
    let want = exp_err.trim();
    let stderr = String::from_utf8_lossy(&p.stderr);
    let ok = p.status.code().unwrap_or(-1) != 0 && stderr.contains(want);
    let detail = if ok {
      None
    } else {
      let out = String::from_utf8_lossy(&p.stdout);
      let last = stderr.lines().chain(out.lines()).last().unwrap_or("?");
      Some(format!(
        "exit={} stderr={}",
        p.status.code().unwrap_or(-1),
        last
      ))
    };
    let status = if ok {
      if xfail { Status::XPass } else { Status::Pass }
    } else {
      Status::Fail
    };
    return (status, detail);
  }

  let stdout = String::from_utf8_lossy(&p.stdout);
  let stderr = String::from_utf8_lossy(&p.stderr);

  let ok = p.status.success() && css_equal(exp_css.as_deref().unwrap_or(""), &stdout);

  let dbg_ok = match &exp_dbg {
    Some(dbg) => dbg_matches(&dbg.lines().collect::<Vec<_>>(), &stderr),
    None => true,
  };

  if ok && dbg_ok && xfail {
    return (Status::XPass, None);
  }
  if ok && dbg_ok {
    return (Status::Pass, None);
  }
  if xfail {
    return (Status::XFail, None);
  }

  let mut detail = Vec::new();
  if !p.status.success() {
    let err = String::from_utf8_lossy(&p.stderr);
    let last = err.lines().last().unwrap_or("?");
    detail.push(format!("exit={}: {}", p.status.code().unwrap_or(-1), last));
  } else if !dbg_ok {
    detail.push(format!(
      "debug mismatch: expected {:?} got {:?}",
      exp_dbg.as_deref().unwrap_or("").lines().collect::<Vec<_>>(),
      dbg_values(&stderr)
    ));
  } else if !css_equal(exp_css.as_deref().unwrap_or(""), &stdout) {
    detail.push("CSS mismatch:".to_string());
    let exp = exp_css.as_deref().unwrap_or("");
    detail.push(format!("  expected: {:?}", &exp[..200.min(exp.len())]));
    detail.push(format!(
      "  actual  : {:?}",
      &stdout[..200.min(stdout.len())]
    ));
  }
  (Status::Fail, Some(detail.join("\n")))
}

fn run_fixture(rel: &str, quiet: bool) -> (Status, Option<String>) {
  let kind = Path::new(rel)
    .extension()
    .and_then(|e| e.to_str())
    .unwrap_or("");
  let (status, detail) = match kind {
    "vcss" | "vass" => run_vcss_fixture(rel),
    "cli" | "int" => {
      let stem = Path::new(ROOT).join(rel).with_extension("");
      let skip_path_str = format!("{}.skip", stem.display());
      let skip_path = Path::new(&skip_path_str);
      if let Some(skip) = read(skip_path) {
        (Status::Skip, Some(skip.trim().to_string()))
      } else {
        run_blocks_fixture(rel, kind)
      }
    }
    "watch" => (
      Status::NotApplicable,
      Some("watch fixtures handled by watch_runner".to_string()),
    ),
    _ => (
      Status::Invalid,
      Some(format!("unknown fixture kind {kind}")),
    ),
  };

  match status {
    Status::Fail => {
      println!("FAIL  {rel}");
      if let Some(d) = &detail
        && !quiet
      {
        for line in d.lines() {
          println!("      {line}");
        }
      }
    }
    Status::Invalid => println!("INVALID {rel}: {}", detail.as_deref().unwrap_or("")),
    Status::Skip | Status::Unsupported | Status::NotApplicable | Status::NotVerified => {
      if !quiet {
        println!("{:<6} {}", status.as_str(), rel);
      }
    }
    Status::XFail | Status::XPass => {
      if !quiet {
        println!("{:<6} {}", status.as_str(), rel);
      }
    }
    Status::Pass => {}
  }
  (status, detail)
}

/// Runs all non-watch fixtures and asserts none fail.
#[test]
fn run_all_fixtures() {
  let quiet = std::env::var("BASS_TEST_QUIET").is_ok();
  let fixtures = discover("");
  let mut pass = 0i32;
  let mut fail = 0i32;
  let mut xfail = 0i32;
  let mut xpass = 0i32;
  let mut skip = 0i32;
  let mut unsupported = 0i32;
  let mut not_applicable = 0i32;
  let mut not_verified = 0i32;
  let mut invalid = 0i32;
  let mut failures: Vec<(String, String)> = Vec::new();

  for rel in &fixtures {
    let kind = Path::new(rel)
      .extension()
      .and_then(|e| e.to_str())
      .unwrap_or("");
    if kind == "watch" {
      continue; // watch fixtures handled by watch_runner
    }
    let (status, detail) = run_fixture(rel, quiet);
    match status {
      Status::Pass => pass += 1,
      Status::Fail => {
        fail += 1;
        if let Some(d) = detail {
          failures.push((rel.clone(), d));
        }
      }
      Status::XFail => xfail += 1,
      Status::XPass => xpass += 1,
      Status::Skip => skip += 1,
      Status::Unsupported => unsupported += 1,
      Status::NotApplicable => not_applicable += 1,
      Status::NotVerified => not_verified += 1,
      Status::Invalid => invalid += 1,
    }
  }

  let total = fixtures.len()
    - fixtures
      .iter()
      .filter(|r| Path::new(r).extension().and_then(|e| e.to_str()) == Some("watch"))
      .count();
  let skip_count = skip + unsupported + not_applicable + not_verified;
  let executed = total.saturating_sub(skip_count as usize);

  println!("\n{}", "=".repeat(60));
  println!("FIXTURES DISCOVERED: {total}   EXECUTED: {executed}");
  if pass > 0 {
    println!("  {:<14} {}", "PASS", pass);
  }
  if fail > 0 {
    println!("  {:<14} {}", "FAIL", fail);
  }
  if xfail > 0 {
    println!("  {:<14} {}", "XFAIL", xfail);
  }
  if xpass > 0 {
    println!("  {:<14} {}", "XPASS", xpass);
  }
  if skip > 0 {
    println!("  {:<14} {}", "SKIP", skip);
  }
  if unsupported > 0 {
    println!("  {:<14} {}", "UNSUPPORTED", unsupported);
  }
  if not_applicable > 0 {
    println!("  {:<14} {}", "NOT-APPLICABLE", not_applicable);
  }
  if not_verified > 0 {
    println!("  {:<14} {}", "NOT-VERIFIED", not_verified);
  }
  if invalid > 0 {
    println!("  {:<14} {}", "INVALID", invalid);
  }
  println!("{}", "=".repeat(60));

  assert!(
    fail == 0 && invalid == 0,
    "{} fixture failures and {} invalid fixtures",
    fail,
    invalid
  );
  if !failures.is_empty() {
    for (rel, detail) in &failures {
      println!("FAILURE DETAIL {rel}: {detail}");
    }
  }
}
