//! Command implementations for xtask.

use anyhow::Context;

pub mod audit;
pub mod bench;
pub mod check;
pub mod clean;
pub mod completions;
pub mod fmt;
pub mod lint;
pub mod package;
pub mod release;
pub mod test;
pub mod verify;
pub mod version;

/// Run `cargo <cmd> <args>` and propagate its exit code.
pub fn run_cargo(cmd: &str, args: &[String]) -> anyhow::Result<()> {
  let status = std::process::Command::new("cargo")
    .arg(cmd)
    .args(args)
    .status()
    .with_context(|| format!("failed to start `cargo {cmd}`"))?;
  if !status.success() {
    anyhow::bail!("`cargo {cmd}` exited with {status}");
  }
  Ok(())
}

/// Ensure `--workspace` is present unless `-p`/`--package` is given.
pub fn with_workspace(args: &[String]) -> Vec<String> {
  if args
    .iter()
    .any(|a| a == "--workspace" || a == "-p" || a == "--package")
  {
    args.to_vec()
  } else {
    let mut v = vec!["--workspace".to_string()];
    v.extend_from_slice(args);
    v
  }
}

/// Return the workspace root directory (two levels above this xtask crate).
fn workspace_root() -> &'static std::path::Path {
  std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
    .ancestors()
    .nth(2)
    .expect("expected workspace root two levels above the xtask crate")
}

/// Read the workspace version from `{manifest_dir}/Cargo.toml`.
pub fn workspace_version(manifest_dir: &std::path::Path) -> anyhow::Result<String> {
  let manifest = std::fs::read_to_string(manifest_dir.join("Cargo.toml"))
    .with_context(|| format!("failed to read root Cargo.toml in {manifest_dir:?}"))?;
  let section = manifest
    .split("[workspace.package]")
    .nth(1)
    .context("missing [workspace.package] section")?;
  for line in section.lines() {
    let t = line.trim();
    if let Some(rest) = t.strip_prefix("version =") {
      let v = rest.trim().trim_matches('"');
      if !v.is_empty() {
        return Ok(v.to_string());
      }
    }
  }
  anyhow::bail!("workspace version not found in [workspace.package]")
}

/// Verify that every publishable crate in the workspace declares the given version.
pub fn check_all_crate_versions(
  expected: &str,
  manifest_dir: &std::path::Path,
) -> anyhow::Result<()> {
  let manifest = std::fs::read_to_string(manifest_dir.join("Cargo.toml"))?;
  let members: Vec<String> = manifest
    .lines()
    .filter_map(|l| {
      let t = l.trim().trim_matches(',');
      if t.starts_with('"') && t.ends_with('"') {
        Some(t[1..t.len() - 1].to_string())
      } else {
        None
      }
    })
    .collect();

  for member in members {
    let path = manifest_dir
      .join(member.trim_end_matches('/'))
      .join("Cargo.toml");
    if !path.exists() {
      continue;
    }
    let text = std::fs::read_to_string(&path)?;
    let mut found = false;
    for line in text.lines() {
      let t = line.trim();
      if t == "version.workspace = true" {
        found = true;
        break;
      }
      if let Some(rest) = t.strip_prefix("version =") {
        let v = rest.trim().trim_matches('"');
        if v != expected {
          anyhow::bail!("crate {member} has version {v}, expected {expected}");
        }
        found = true;
        break;
      }
    }
    if !found {
      anyhow::bail!("crate {member} has no version declaration");
    }
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_with_workspace_adds_flag() {
    let args = vec!["--locked".to_string()];
    let result = with_workspace(&args);
    assert_eq!(
      result,
      vec!["--workspace".to_string(), "--locked".to_string()]
    );
  }

  #[test]
  fn test_with_workspace_keeps_existing() {
    let args = vec!["--workspace".to_string(), "--locked".to_string()];
    let result = with_workspace(&args);
    assert_eq!(
      result,
      vec!["--workspace".to_string(), "--locked".to_string()]
    );
  }

  #[test]
  fn test_with_workspace_keeps_package() {
    let args = vec!["-p".to_string(), "vass-core".to_string()];
    let result = with_workspace(&args);
    assert_eq!(result, vec!["-p".to_string(), "vass-core".to_string()]);
  }

  #[test]
  fn test_workspace_version() {
    let v = workspace_version(workspace_root()).expect("workspace version should be readable");
    assert!(!v.is_empty());
    assert_eq!(v, "1.0.0");
  }

  #[test]
  fn test_run_cargo_success() {
    // `cargo version` always succeeds and exercises the success path of run_cargo.
    run_cargo("version", &[]).expect("cargo version should succeed");
  }

  #[test]
  fn test_run_cargo_failure() {
    // An invalid flag makes cargo exit non-zero, exercising the bail path.
    let err = run_cargo("check", &["--__no_such_vass_flag__".to_string()]).unwrap_err();
    assert!(
      err.to_string().contains("exited with"),
      "expected non-zero exit error, got: {err}"
    );
  }

  #[test]
  fn test_check_all_crate_versions() {
    check_all_crate_versions("1.0.0", workspace_root()).expect("all crates should declare 1.0.0");
  }
}
