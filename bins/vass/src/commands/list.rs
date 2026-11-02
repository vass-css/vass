//! `vass list` — introspect the built-in function library.
//!
//! `modules` lists the built-in module namespaces, `functions <mod>` the
//! functions of one module (or all modules), `globals` the global
//! functions, `variables <mod>` the module's exported variables.

use clap::Args as ClapArgs;

/// List built-in modules, functions and variables
#[derive(ClapArgs, Debug)]
pub struct Args {
  /// What to list: modules | functions | globals | variables
  #[arg(value_name = "WHAT", default_value = "modules")]
  pub what: String,

  /// Module name (for `functions` / `variables`); omit for all modules
  #[arg(value_name = "MODULE")]
  pub module: Option<String>,
}

use vass_evaluator::builtins::{
  BUILTIN_MODULES, global_fn_names, is_builtin_module, module_function_names, module_variables,
};

/// Run the list command
pub async fn run(args: Args) -> anyhow::Result<()> {
  match args.what.as_str() {
    "modules" => {
      for m in BUILTIN_MODULES {
        println!("{m}");
      }
    }
    "functions" => match &args.module {
      Some(m) => {
        ensure_module(m)?;
        for f in module_function_names(m) {
          println!("{f}");
        }
      }
      None => {
        for m in BUILTIN_MODULES {
          println!("[{m}]");
          for f in module_function_names(m) {
            println!("  {f}");
          }
        }
      }
    },
    "variables" => match &args.module {
      Some(m) => {
        ensure_module(m)?;
        for (name, value) in module_variables(m) {
          println!("{name} = {}", value.inspect());
        }
      }
      None => {
        for m in BUILTIN_MODULES {
          let vars = module_variables(m);
          if vars.is_empty() {
            continue;
          }
          println!("[{m}]");
          for (name, value) in vars {
            println!("  {name} = {}", value.inspect());
          }
        }
      }
    },
    "globals" => {
      for f in global_fn_names() {
        println!("{f}");
      }
    }
    other => {
      anyhow::bail!("unknown listing `{other}` (expected modules, functions, globals or variables)")
    }
  }
  Ok(())
}

fn ensure_module(m: &str) -> anyhow::Result<()> {
  if is_builtin_module(m) {
    Ok(())
  } else {
    anyhow::bail!(
      "unknown built-in module `{m}` (available: {})",
      BUILTIN_MODULES.join(", ")
    )
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_ensure_module_valid() {
    assert!(ensure_module("math").is_ok());
    assert!(ensure_module("color").is_ok());
    assert!(ensure_module("string").is_ok());
  }

  #[test]
  fn test_ensure_module_invalid() {
    assert!(ensure_module("nonexistent_module").is_err());
  }

  #[tokio::test]
  async fn test_list_run_modules() {
    let result = run(Args {
      what: "modules".to_string(),
      module: None,
    })
    .await;
    assert!(result.is_ok());
  }

  #[tokio::test]
  async fn test_list_run_functions_all() {
    let result = run(Args {
      what: "functions".to_string(),
      module: None,
    })
    .await;
    assert!(result.is_ok());
  }

  #[tokio::test]
  async fn test_list_run_functions_module() {
    let result = run(Args {
      what: "functions".to_string(),
      module: Some("math".to_string()),
    })
    .await;
    assert!(result.is_ok());
  }

  #[tokio::test]
  async fn test_list_run_globals() {
    let result = run(Args {
      what: "globals".to_string(),
      module: None,
    })
    .await;
    assert!(result.is_ok());
  }

  #[tokio::test]
  async fn test_list_run_variables_all() {
    let result = run(Args {
      what: "variables".to_string(),
      module: None,
    })
    .await;
    assert!(result.is_ok());
  }

  #[tokio::test]
  async fn test_list_run_variables_module() {
    let result = run(Args {
      what: "variables".to_string(),
      module: Some("math".to_string()),
    })
    .await;
    assert!(result.is_ok());
  }

  #[tokio::test]
  async fn test_list_run_unknown() {
    let result = run(Args {
      what: "unknown_what".to_string(),
      module: None,
    })
    .await;
    assert!(result.is_err());
  }
}
