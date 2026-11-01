//! Selector helpers

/// Validate a captured selector line is non-empty
pub(crate) fn validate_selector(selector: &str) -> Result<(), String> {
  if selector.trim().is_empty() {
    Err("Style rules must have a selector.".to_string())
  } else {
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_validate() {
    assert!(validate_selector(".a").is_ok());
    assert!(validate_selector("  ").is_err());
  }
}
