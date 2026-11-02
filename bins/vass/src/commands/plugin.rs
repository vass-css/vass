//! Plugin command

use clap::{Args as ClapArgs, Subcommand};

/// Manage plugins
#[derive(ClapArgs, Debug)]
pub struct Args {
  #[command(subcommand)]
  command: PluginCommands,
}

#[derive(Subcommand, Debug)]
enum PluginCommands {
  /// List installed plugins
  List,
  /// Install a plugin
  Install { name: String },
  /// Remove a plugin
  Remove { name: String },
  /// Update plugins
  Update,
}

/// Run the plugin command
pub async fn run(args: Args) -> anyhow::Result<()> {
  match args.command {
    PluginCommands::List => {
      println!("Installed plugins:");
      println!("  (none)");
    }
    PluginCommands::Install { name } => {
      println!("Installing plugin '{}'...", name);
      println!("Plugin system not yet implemented.");
    }
    PluginCommands::Remove { name } => {
      println!("Removing plugin '{}'...", name);
      println!("Plugin system not yet implemented.");
    }
    PluginCommands::Update => {
      println!("Updating plugins...");
      println!("Plugin system not yet implemented.");
    }
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn test_plugin_run_list() {
    let result = run(Args {
      command: PluginCommands::List,
    })
    .await;
    assert!(result.is_ok());
  }

  #[tokio::test]
  async fn test_plugin_run_install() {
    let result = run(Args {
      command: PluginCommands::Install {
        name: "test-plugin".to_string(),
      },
    })
    .await;
    assert!(result.is_ok());
  }

  #[tokio::test]
  async fn test_plugin_run_remove() {
    let result = run(Args {
      command: PluginCommands::Remove {
        name: "test-plugin".to_string(),
      },
    })
    .await;
    assert!(result.is_ok());
  }

  #[tokio::test]
  async fn test_plugin_run_update() {
    let result = run(Args {
      command: PluginCommands::Update,
    })
    .await;
    assert!(result.is_ok());
  }
}
