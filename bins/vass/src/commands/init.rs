//! Init command

use clap::Args as ClapArgs;
use std::fs;
use std::path::Path;

/// Initialize a new VASS project
#[derive(ClapArgs, Debug)]
pub struct Args {
  /// Project name
  #[arg(value_name = "NAME")]
  pub name: String,

  /// Use VCSS-style syntax (.vcss) instead of indented (.vass)
  #[arg(long)]
  pub vcss: bool,
}

/// Run the init command
pub async fn run(args: Args) -> anyhow::Result<()> {
  let project_path = Path::new(&args.name);

  // Create project directory
  fs::create_dir_all(project_path)?;
  fs::create_dir_all(project_path.join("src"))?;
  fs::create_dir_all(project_path.join("dist"))?;

  // Create main file
  let ext = if args.vcss { "vcss" } else { "vass" };
  let main_file = project_path.join("src").join(format!("main.{}", ext));

  let content = if args.vcss {
    r#"// Main VASS stylesheet
// Project: {name}

$primary-color: #3498db;
$secondary-color: #2ecc71;
$spacing: 1rem;

body {
    margin: 0;
    padding: $spacing;
}

.container {
    max-width: 1200px;
    margin: 0 auto;
}
"#
    .replace("{name}", &args.name)
  } else {
    r#"// Main VASS stylesheet
// Project: {name}

$primary-color: #3498db
$secondary-color: #2ecc71
$spacing: 1rem

body
  margin: 0
  padding: $spacing

.container
  max-width: 1200px
  margin: 0 auto
"#
    .replace("{name}", &args.name)
  };

  fs::write(main_file, content)?;

  // Create vass.toml config
  let config = format!(
    r#"[project]
name = "{}"
version = "0.1.0"

[compile]
input = "src/main.{}"
output = "dist/main.css"
style = "expanded"
source-map = true
"#,
    args.name, ext
  );

  fs::write(project_path.join("vass.toml"), config)?;

  println!("Created project '{}' at ./{}", args.name, args.name);
  println!();
  println!("Next steps:");
  println!("  cd {}", args.name);
  println!("  vass compile");

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn test_init_run_vass() {
    let result = run(Args {
      name: "test_init_project_vass".to_string(),
      vcss: false,
    })
    .await;
    assert!(result.is_ok());
    // Clean up
    let _ = std::fs::remove_dir_all("test_init_project_vass");
  }

  #[tokio::test]
  async fn test_init_run_vcss() {
    let result = run(Args {
      name: "test_init_project_vcss".to_string(),
      vcss: true,
    })
    .await;
    assert!(result.is_ok());
    // Clean up
    let _ = std::fs::remove_dir_all("test_init_project_vcss");
  }
}
