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

/// Read the workspace version from the root Cargo.toml.
pub fn workspace_version() -> anyhow::Result<String> {
  let manifest = std::fs::read_to_string("Cargo.toml").context("failed to read root Cargo.toml")?;
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
pub fn check_all_crate_versions(expected: &str) -> anyhow::Result<()> {
  let manifest = std::fs::read_to_string("Cargo.toml")?;
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
    let path = format!("{}/Cargo.toml", member.trim_end_matches('/'));
    if !std::path::Path::new(&path).exists() {
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
